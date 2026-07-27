//! What a host knows about itself and about the devices it has admitted.
//!
//! The identity is deliberately independent of address, port, and hostname:
//! all three change, and a companion cannot tell "my host moved" from "a
//! different machine is here now" if identity is the thing that moved.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::crypto::{is_hex, random_hex};
use crate::protocol::{
    derive_device_secret, pair_request_mac_matches, pair_response_mac, sanitise_device_name,
    DEVICE_NAME_MAX_LENGTH, PAIRING_LIFETIME_SECONDS, PAIRING_SECRET_BYTES, PROTOCOL_VERSION,
};

const HOST_ID_BYTES: usize = 16;
const HOST_ID_FILE: &str = "host-id";
const HOST_NAME_FILE: &str = "host-name";
const DEVICES_FILE: &str = "devices.json";
const OWNER_ONLY: u32 = 0o600;

/// How long the server thread waits for someone to answer the approval sheet
/// before treating silence as refusal. Connections are handled sequentially, so
/// this also bounds how long one pairing attempt can hold up the server.
const APPROVAL_TIMEOUT: Duration = Duration::from_secs(60);

pub const PLATFORM: &str = if cfg!(target_os = "macos") {
    "macos"
} else {
    "linux"
};

pub fn goghmode_dir(home_dir: &Path) -> PathBuf {
    home_dir.join(".goghmode")
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostIdentity {
    pub host_id: String,
    pub display_name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Device {
    #[serde(rename = "deviceId")]
    pub device_id: String,
    #[serde(rename = "deviceName")]
    pub device_name: String,
    pub platform: String,
    pub secret: String,
    #[serde(rename = "pairedAt")]
    pub paired_at: u128,
    #[serde(rename = "lastSeenAt", default)]
    pub last_seen_at: u128,
    /// The newest upload timestamp accepted from this device. Persisting it is
    /// what makes replay protection survive a restart, which an in-memory set
    /// of seen nonces does not.
    #[serde(rename = "lastAcceptedTimestamp", default)]
    pub last_accepted_timestamp: u128,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub devices: Vec<Device>,
    /// Starts true so an install that has never paired keeps working exactly as
    /// before. The first successful pairing turns it off.
    #[serde(rename = "legacyUploadsEnabled", default = "enabled_by_default")]
    pub legacy_uploads_enabled: bool,
}

fn enabled_by_default() -> bool {
    true
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            devices: Vec::new(),
            legacy_uploads_enabled: true,
        }
    }
}

impl Registry {
    pub fn device(&self, device_id: &str) -> Option<&Device> {
        self.devices
            .iter()
            .find(|device| device.device_id == device_id)
    }

    fn device_mut(&mut self, device_id: &str) -> Option<&mut Device> {
        self.devices
            .iter_mut()
            .find(|device| device.device_id == device_id)
    }
}

/// What the companion needs in order to pair, as it appears in the QR code.
///
/// `addresses` is plural on purpose. Only one entry is filled in today, but a
/// host with several interfaces should offer all of them, and widening a list
/// later is not a wire change whereas turning a string into a list would be.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PairingPayload {
    pub v: u32,
    #[serde(rename = "hostId")]
    pub host_id: String,
    pub name: String,
    pub platform: String,
    pub addresses: Vec<String>,
    #[serde(rename = "pairingSecret")]
    pub pairing_secret: String,
}

/// A pairing request that has proved it holds the shown secret and is now
/// waiting for a person to say yes.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingPairing {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub peer_address: String,
    pub decision: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PairingState {
    Idle,
    Armed {
        secret: String,
        expires_at: Instant,
    },
    Pending {
        secret: String,
        request: PendingPairing,
    },
}

pub struct HostState {
    pub identity: HostIdentity,
    pub registry: Registry,
    pub pairing: PairingState,
    pub last_refusal: Option<Refusal>,
    registry_path: PathBuf,
}

/// Why the most recent signed request was turned away. The wire answer stays a
/// bare 401 so nothing is learned from which check rejected it; this is the
/// same event told to the person sitting at the host, who otherwise has no way
/// to tell a stale address from a drifting clock from a revoked device.
///
/// Deliberately not persisted: it describes a link right now, not a fact about
/// the registry, and a reason surviving a restart would outlive its truth.
#[derive(Clone, Debug, PartialEq)]
pub struct Refusal {
    pub at: u128,
    pub reason: String,
}

/// Shared between the server thread and the user interface. The condition
/// variable is how a request waiting on the approval sheet learns that someone
/// answered it.
pub struct SharedHost {
    state: Mutex<HostState>,
    approval: Condvar,
}

pub type Host = Arc<SharedHost>;

#[derive(Debug, PartialEq)]
pub enum PairOutcome {
    Approved {
        host_id: String,
        /// Computed while the pairing secret is still in hand, because it is
        /// burned before this returns. It is what lets the companion prove the
        /// machine that answered is the one whose screen it scanned.
        pair_response_mac: String,
    },
    Refused,
}

impl SharedHost {
    pub fn load(goghmode_dir: &Path) -> anyhow::Result<Host> {
        fs::create_dir_all(goghmode_dir)?;
        let identity = load_or_create_identity(goghmode_dir)?;
        let registry_path = goghmode_dir.join(DEVICES_FILE);
        let registry = load_registry(&registry_path);

        Ok(Arc::new(Self {
            state: Mutex::new(HostState {
                identity,
                registry,
                pairing: PairingState::Idle,
                last_refusal: None,
                registry_path,
            }),
            approval: Condvar::new(),
        }))
    }

    pub fn identity(&self) -> HostIdentity {
        self.locked().identity.clone()
    }

    pub fn host_id(&self) -> String {
        self.locked().identity.host_id.clone()
    }

    pub fn devices(&self) -> Vec<Device> {
        self.locked().registry.devices.clone()
    }

    pub fn legacy_uploads_enabled(&self) -> bool {
        self.locked().registry.legacy_uploads_enabled
    }

    pub fn set_legacy_uploads_enabled(&self, enabled: bool) -> anyhow::Result<()> {
        let mut state = self.locked();
        state.registry.legacy_uploads_enabled = enabled;
        state.save_registry()
    }

    pub fn set_display_name(&self, name: &str, goghmode_dir: &Path) -> anyhow::Result<()> {
        let name = sanitise_display_name(name);
        self.locked().identity.display_name = name.clone();
        write_owner_only(&goghmode_dir.join(HOST_NAME_FILE), name.as_bytes())
    }

    pub fn revoke(&self, device_id: &str) -> anyhow::Result<()> {
        let mut state = self.locked();
        state
            .registry
            .devices
            .retain(|device| device.device_id != device_id);
        state.save_registry()
    }

    /// Shows a pairing secret and returns everything the companion needs to use
    /// it. Arming replaces any previous secret, so only one is ever live.
    pub fn arm_pairing(&self, addresses: Vec<String>) -> anyhow::Result<PairingPayload> {
        let secret = random_hex(PAIRING_SECRET_BYTES)?;
        let mut state = self.locked();
        state.pairing = PairingState::Armed {
            secret: secret.clone(),
            expires_at: Instant::now() + Duration::from_secs(PAIRING_LIFETIME_SECONDS),
        };

        Ok(PairingPayload {
            v: PROTOCOL_VERSION,
            host_id: state.identity.host_id.clone(),
            name: state.identity.display_name.clone(),
            platform: PLATFORM.to_owned(),
            addresses,
            pairing_secret: secret,
        })
    }

    pub fn cancel_pairing(&self) {
        let mut state = self.locked();
        if let PairingState::Pending { request, .. } = &mut state.pairing {
            request.decision = Some(false);
            self.approval.notify_all();
            return;
        }
        state.pairing = PairingState::Idle;
    }

    pub fn pairing_state(&self) -> PairingState {
        let mut state = self.locked();
        state.expire_armed_pairing();
        state.pairing.clone()
    }

    /// Answers the approval sheet. Wakes the server thread that is blocked on
    /// the request.
    pub fn decide_pending_pairing(&self, approved: bool) {
        let mut state = self.locked();
        if let PairingState::Pending { request, .. } = &mut state.pairing {
            request.decision = Some(approved);
        }
        self.approval.notify_all();
    }

    /// The server side of pairing. Verifies that the caller holds the shown
    /// secret, then blocks until a person answers or the wait times out.
    ///
    /// A caller that cannot produce the signature never reaches the approval
    /// sheet, so nothing on the network can raise a prompt on someone's screen.
    pub fn complete_pairing(
        &self,
        device_id: &str,
        raw_device_name: &str,
        platform: &str,
        peer_address: &str,
        pair_mac: &str,
    ) -> PairOutcome {
        let device_name = sanitise_device_name(raw_device_name);
        let mut state = self.locked();
        state.expire_armed_pairing();

        let PairingState::Armed { secret, .. } = &state.pairing else {
            return PairOutcome::Refused;
        };
        let secret = secret.clone();
        let host_id = state.identity.host_id.clone();
        if !pair_request_mac_matches(&secret, &host_id, device_id, &device_name, pair_mac) {
            return PairOutcome::Refused;
        }

        state.pairing = PairingState::Pending {
            secret: secret.clone(),
            request: PendingPairing {
                device_id: device_id.to_owned(),
                device_name: device_name.clone(),
                platform: platform.to_owned(),
                peer_address: peer_address.to_owned(),
                decision: None,
            },
        };

        let (mut state, timed_out) = self
            .approval
            .wait_timeout_while(state, APPROVAL_TIMEOUT, |state| {
                matches!(
                    &state.pairing,
                    PairingState::Pending { request, .. } if request.decision.is_none()
                )
            })
            .expect("the host state mutex is never poisoned");

        let approved = match &state.pairing {
            PairingState::Pending { request, .. } => request.decision.unwrap_or(false),
            // Something else replaced the pending request while we waited.
            _ => false,
        };
        state.pairing = PairingState::Idle;

        if timed_out.timed_out() || !approved {
            return PairOutcome::Refused;
        }

        let device_secret = derive_device_secret(&secret, device_id);
        let now = unix_millis();
        state.registry.devices.retain(|device| device.device_id != device_id);
        state.registry.devices.push(Device {
            device_id: device_id.to_owned(),
            device_name,
            platform: platform.to_owned(),
            secret: device_secret.clone(),
            paired_at: now,
            last_seen_at: now,
            last_accepted_timestamp: 0,
        });
        // An authenticated route beside an anonymous one accepting writes to the
        // same directory is not an improvement, so the old door closes here.
        state.registry.legacy_uploads_enabled = false;
        let _ = state.save_registry();

        PairOutcome::Approved {
            pair_response_mac: pair_response_mac(&secret, &host_id, device_id),
            host_id,
        }
    }

    pub fn device_secret(&self, device_id: &str) -> Option<String> {
        self.locked()
            .registry
            .device(device_id)
            .map(|device| device.secret.clone())
    }

    /// Accepts an upload's timestamp only if it moves forward for this device,
    /// and records it in the same step so the check cannot be raced.
    pub fn accept_timestamp(&self, device_id: &str, timestamp_millis: u128) -> bool {
        let mut state = self.locked();
        let Some(device) = state.registry.device_mut(device_id) else {
            return false;
        };
        if timestamp_millis <= device.last_accepted_timestamp {
            return false;
        }
        device.last_accepted_timestamp = timestamp_millis;
        device.last_seen_at = unix_millis();
        // ponytail: one small JSON write per accepted upload, alongside three
        // export files that are already being written. Batch it if it ever shows.
        let _ = state.save_registry();
        true
    }

    /// Remembers why a signed request was turned away, and says it on stderr
    /// for a host started from a terminal. An app bundle has nowhere to send
    /// stderr, which is why the interface reads this back rather than relying
    /// on the printed line.
    pub fn record_refusal(&self, reason: String) {
        eprintln!("goghmode: refused a signed request: {reason}");
        self.locked().last_refusal = Some(Refusal {
            at: unix_millis(),
            reason,
        });
    }

    pub fn last_refusal(&self) -> Option<Refusal> {
        self.locked().last_refusal.clone()
    }

    /// A device that gets through clears the complaint, so a fixed link stops
    /// showing the failure that preceded it.
    pub fn clear_refusal(&self) {
        self.locked().last_refusal = None;
    }

    fn locked(&self) -> std::sync::MutexGuard<'_, HostState> {
        self.state
            .lock()
            .expect("the host state mutex is never poisoned")
    }
}

impl HostState {
    fn expire_armed_pairing(&mut self) {
        if let PairingState::Armed { expires_at, .. } = &self.pairing {
            if Instant::now() >= *expires_at {
                self.pairing = PairingState::Idle;
            }
        }
    }

    fn save_registry(&self) -> anyhow::Result<()> {
        let serialised = serde_json::to_string_pretty(&self.registry)?;
        write_owner_only(&self.registry_path, serialised.as_bytes())
    }
}

fn load_registry(path: &Path) -> Registry {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn load_or_create_identity(goghmode_dir: &Path) -> anyhow::Result<HostIdentity> {
    let host_id_path = goghmode_dir.join(HOST_ID_FILE);
    let host_id = match fs::read_to_string(&host_id_path) {
        Ok(existing) if is_hex(existing.trim(), HOST_ID_BYTES) => existing.trim().to_owned(),
        // A corrupt or truncated file is replaced rather than being a fatal
        // error: an unreadable identity should cost a re-pair, not a host that
        // refuses to start.
        _ => {
            let host_id = random_hex(HOST_ID_BYTES)?;
            write_owner_only(&host_id_path, host_id.as_bytes())?;
            host_id
        }
    };

    let host_name_path = goghmode_dir.join(HOST_NAME_FILE);
    let display_name = match fs::read_to_string(&host_name_path) {
        Ok(existing) if !existing.trim().is_empty() => sanitise_display_name(&existing),
        _ => {
            let name = system_hostname();
            write_owner_only(&host_name_path, name.as_bytes())?;
            name
        }
    };

    Ok(HostIdentity {
        host_id,
        display_name,
    })
}

fn sanitise_display_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|character| !character.is_control())
        .take(DEVICE_NAME_MAX_LENGTH)
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "GoghMode host".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// `hostname` exists on both target platforms. There is no standard-library
/// call for this, and pulling in a crate for one string is not worth it.
fn system_hostname() -> String {
    let name = Command::new("hostname")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_default();
    sanitise_display_name(name.trim().trim_end_matches(".local"))
}

fn write_owner_only(path: &Path, contents: &[u8]) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, contents)?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(OWNER_ONLY))?;
    fs::rename(&temporary, path)?;
    Ok(())
}

pub fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}
