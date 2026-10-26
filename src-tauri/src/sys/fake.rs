//! In-memory OS backends for unit tests, with failure injection.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use super::*;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn norm(path: &str) -> String {
    path.trim_matches('\\').to_ascii_lowercase()
}

fn parent(path: &str) -> Option<&str> {
    path.rfind('\\').map(|i| &path[..i])
}

// ---- Registry ---------------------------------------------------------------

#[derive(Default)]
struct RegState {
    keys: BTreeSet<(Hive, String)>,
    values: BTreeMap<(Hive, String, String), RegData>,
}

#[derive(Default)]
pub struct FakeRegistry {
    state: Mutex<RegState>,
    /// Writes whose `HIVE\path\name` contains one of these fail with AccessDenied.
    fail_writes: Mutex<Vec<String>>,
    /// Writes to these targets "succeed" but don't stick, like a value Windows resets.
    ignore_writes: Mutex<Vec<String>>,
    writes: Mutex<u32>,
}

impl FakeRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Seeds a value without counting it as a write.
    pub fn seed(&self, hive: Hive, path: &str, name: &str, data: RegData) {
        let mut st = lock(&self.state);
        Self::create_keys(&mut st, hive, &norm(path));
        st.values
            .insert((hive, norm(path), name.to_ascii_lowercase()), data);
    }

    pub fn seed_key(&self, hive: Hive, path: &str) {
        Self::create_keys(&mut lock(&self.state), hive, &norm(path));
    }

    pub fn fail_writes_to(&self, pattern: &str) {
        lock(&self.fail_writes).push(pattern.to_ascii_lowercase());
    }

    pub fn clear_failures(&self) {
        lock(&self.fail_writes).clear();
    }

    pub fn ignore_writes_to(&self, pattern: &str) {
        lock(&self.ignore_writes).push(pattern.to_ascii_lowercase());
    }

    pub fn write_count(&self) -> u32 {
        *lock(&self.writes)
    }

    pub fn value(&self, hive: Hive, path: &str, name: &str) -> Option<RegData> {
        self.get(hive, path, name).unwrap()
    }

    pub fn has_key(&self, hive: Hive, path: &str) -> bool {
        self.key_exists(hive, path).unwrap()
    }

    fn create_keys(st: &mut RegState, hive: Hive, path: &str) {
        let mut current = Some(path);
        while let Some(p) = current {
            st.keys.insert((hive, p.to_owned()));
            current = parent(p);
        }
    }

    fn check_write(&self, hive: Hive, path: &str, name: &str) -> SysResult<bool> {
        *lock(&self.writes) += 1;
        let target = format!("{hive}\\{path}\\{name}").to_ascii_lowercase();
        if lock(&self.fail_writes).iter().any(|p| target.contains(p)) {
            return Err(SysError::new(
                SysErrorKind::AccessDenied,
                "write registry",
                target,
                "Access is denied.",
            ));
        }
        Ok(lock(&self.ignore_writes).iter().any(|p| target.contains(p)))
    }
}

impl Registry for FakeRegistry {
    fn get(&self, hive: Hive, path: &str, name: &str) -> SysResult<Option<RegData>> {
        let st = lock(&self.state);
        Ok(st
            .values
            .get(&(hive, norm(path), name.to_ascii_lowercase()))
            .cloned())
    }

    fn key_exists(&self, hive: Hive, path: &str) -> SysResult<bool> {
        Ok(lock(&self.state).keys.contains(&(hive, norm(path))))
    }

    fn set(&self, hive: Hive, path: &str, name: &str, data: &RegData) -> SysResult<()> {
        if self.check_write(hive, path, name)? {
            return Ok(());
        }
        let mut st = lock(&self.state);
        Self::create_keys(&mut st, hive, &norm(path));
        st.values
            .insert((hive, norm(path), name.to_ascii_lowercase()), data.clone());
        Ok(())
    }

    fn delete_value(&self, hive: Hive, path: &str, name: &str) -> SysResult<()> {
        if self.check_write(hive, path, name)? {
            return Ok(());
        }
        lock(&self.state)
            .values
            .remove(&(hive, norm(path), name.to_ascii_lowercase()));
        Ok(())
    }

    fn delete_key_if_empty(&self, hive: Hive, path: &str) -> SysResult<bool> {
        self.check_write(hive, path, "")?;
        let key = norm(path);
        let mut st = lock(&self.state);
        if !st.keys.contains(&(hive, key.clone())) {
            return Ok(false);
        }
        let prefix = format!("{key}\\");
        let has_values = st.values.keys().any(|(h, p, _)| *h == hive && *p == key);
        let has_children = st
            .keys
            .iter()
            .any(|(h, p)| *h == hive && p.starts_with(&prefix));
        if has_values || has_children {
            return Ok(false);
        }
        st.keys.remove(&(hive, key));
        Ok(true)
    }

    fn subkeys(&self, hive: Hive, path: &str) -> SysResult<Vec<String>> {
        let key = norm(path);
        let prefix = format!("{key}\\");
        let st = lock(&self.state);
        Ok(st
            .keys
            .iter()
            .filter(|(h, p)| {
                *h == hive && p.starts_with(&prefix) && !p[prefix.len()..].contains('\\')
            })
            .map(|(_, p)| p[prefix.len()..].to_owned())
            .collect())
    }
}

// ---- Services ---------------------------------------------------------------

#[derive(Default)]
pub struct FakeServices {
    services: Mutex<HashMap<String, ServiceState>>,
    fail: Mutex<Vec<String>>,
}

impl FakeServices {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn install(&self, name: &str, start: StartType, running: bool) {
        lock(&self.services).insert(name.to_ascii_lowercase(), ServiceState { start, running });
    }

    pub fn fail_changes_to(&self, name: &str) {
        lock(&self.fail).push(name.to_ascii_lowercase());
    }

    pub fn state(&self, name: &str) -> Option<ServiceState> {
        lock(&self.services)
            .get(&name.to_ascii_lowercase())
            .copied()
    }

    fn modify(
        &self,
        op: &'static str,
        name: &str,
        f: impl FnOnce(&mut ServiceState),
    ) -> SysResult<()> {
        let key = name.to_ascii_lowercase();
        if lock(&self.fail).contains(&key) {
            return Err(SysError::new(
                SysErrorKind::AccessDenied,
                op,
                name,
                "Access is denied.",
            ));
        }
        let mut services = lock(&self.services);
        let state = services.get_mut(&key).ok_or_else(|| {
            SysError::new(
                SysErrorKind::NotFound,
                op,
                name,
                "The service does not exist.",
            )
        })?;
        f(state);
        Ok(())
    }
}

impl Services for FakeServices {
    fn query(&self, name: &str) -> SysResult<Option<ServiceState>> {
        Ok(self.state(name))
    }

    fn set_start(&self, name: &str, start: StartType) -> SysResult<()> {
        self.modify("configure service", name, |s| s.start = start)
    }

    fn stop(&self, name: &str) -> SysResult<()> {
        self.modify("stop service", name, |s| s.running = false)
    }

    fn start(&self, name: &str) -> SysResult<()> {
        self.modify("start service", name, |s| s.running = true)
    }
}

// ---- Restore points ---------------------------------------------------------

pub struct FakeRestorePoints {
    pub enabled: Mutex<Option<bool>>,
    pub points: Mutex<Vec<RestorePoint>>,
    pub fail_create: Mutex<Option<SysError>>,
}

impl FakeRestorePoints {
    pub fn new(enabled: Option<bool>) -> Arc<Self> {
        Arc::new(Self {
            enabled: Mutex::new(enabled),
            points: Mutex::new(Vec::new()),
            fail_create: Mutex::new(None),
        })
    }

    pub fn count(&self) -> usize {
        lock(&self.points).len()
    }
}

impl RestorePoints for FakeRestorePoints {
    fn protection_enabled(&self) -> SysResult<Option<bool>> {
        Ok(*lock(&self.enabled))
    }

    fn enable_protection(&self) -> SysResult<()> {
        *lock(&self.enabled) = Some(true);
        Ok(())
    }

    fn create(&self, description: &str) -> SysResult<RestorePoint> {
        if let Some(e) = lock(&self.fail_create).clone() {
            return Err(e);
        }
        if *lock(&self.enabled) == Some(false) {
            return Err(SysError::new(
                SysErrorKind::Unsupported,
                "create restore point",
                "C:",
                "System Protection is turned off.",
            ));
        }
        let mut points = lock(&self.points);
        let point = RestorePoint {
            sequence: points.len() as u32 + 1,
            description: description.to_owned(),
            created: "20261026120000.000000-000".into(),
        };
        points.push(point.clone());
        Ok(point)
    }

    fn list(&self) -> SysResult<Vec<RestorePoint>> {
        Ok(lock(&self.points).clone())
    }
}

// ---- Power ------------------------------------------------------------------

type SettingKey = (String, String, String, PowerSource);

pub struct FakePower {
    pub schemes: Mutex<Vec<PowerScheme>>,
    /// Built-in templates that can be duplicated even when not listed (e.g. Ultimate Performance).
    pub templates: Mutex<Vec<String>>,
    pub active: Mutex<String>,
    pub settings: Mutex<HashMap<SettingKey, u32>>,
    pub battery: Mutex<bool>,
    /// Writes to settings whose GUID is listed here fail.
    pub fail_settings: Mutex<Vec<String>>,
}

pub const BALANCED: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";

impl FakePower {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            schemes: Mutex::new(vec![PowerScheme {
                guid: BALANCED.into(),
                name: "Balanced".into(),
            }]),
            templates: Mutex::new(Vec::new()),
            active: Mutex::new(BALANCED.into()),
            settings: Mutex::new(HashMap::new()),
            battery: Mutex::new(false),
            fail_settings: Mutex::new(Vec::new()),
        })
    }

    pub fn add_template(&self, guid: &str) {
        lock(&self.templates).push(guid.into());
    }

    pub fn value(
        &self,
        scheme: &str,
        subgroup: &str,
        setting: &str,
        source: PowerSource,
    ) -> Option<u32> {
        lock(&self.settings)
            .get(&(scheme.into(), subgroup.into(), setting.into(), source))
            .copied()
    }

    fn missing(op: &'static str, guid: &str) -> SysError {
        SysError::new(
            SysErrorKind::NotFound,
            op,
            guid,
            "The power plan doesn't exist.",
        )
    }
}

impl Power for FakePower {
    fn schemes(&self) -> SysResult<Vec<PowerScheme>> {
        Ok(lock(&self.schemes).clone())
    }

    fn active(&self) -> SysResult<String> {
        Ok(lock(&self.active).clone())
    }

    fn set_active(&self, guid: &str) -> SysResult<()> {
        if !self.exists(guid)? {
            return Err(Self::missing("switch power plan", guid));
        }
        *lock(&self.active) = guid.into();
        Ok(())
    }

    fn exists(&self, guid: &str) -> SysResult<bool> {
        Ok(lock(&self.schemes).iter().any(|s| s.guid == guid))
    }

    fn duplicate(&self, source: &str, dest: &str) -> SysResult<()> {
        let listed = self.exists(source)?;
        if !listed && !lock(&self.templates).iter().any(|t| t == source) {
            return Err(Self::missing("create power plan", source));
        }
        let mut settings = lock(&self.settings);
        let copied: Vec<(SettingKey, u32)> = settings
            .iter()
            .filter(|((s, ..), _)| s == source)
            .map(|((_, g, k, src), v)| ((dest.to_owned(), g.clone(), k.clone(), *src), *v))
            .collect();
        settings.extend(copied);
        lock(&self.schemes).push(PowerScheme {
            guid: dest.into(),
            name: format!("Copy of {source}"),
        });
        Ok(())
    }

    fn delete(&self, guid: &str) -> SysResult<()> {
        if *lock(&self.active) == guid {
            return Err(SysError::new(
                SysErrorKind::Other,
                "delete power plan",
                guid,
                "The plan is active.",
            ));
        }
        lock(&self.schemes).retain(|s| s.guid != guid);
        lock(&self.settings).retain(|(s, ..), _| s != guid);
        Ok(())
    }

    fn rename(&self, guid: &str, name: &str, _description: &str) -> SysResult<()> {
        let mut schemes = lock(&self.schemes);
        let scheme = schemes
            .iter_mut()
            .find(|s| s.guid == guid)
            .ok_or_else(|| Self::missing("rename power plan", guid))?;
        scheme.name = name.into();
        Ok(())
    }

    fn read(
        &self,
        scheme: &str,
        subgroup: &str,
        setting: &str,
        source: PowerSource,
    ) -> SysResult<Option<u32>> {
        if !self.exists(scheme)? {
            return Ok(None);
        }
        Ok(self.value(scheme, subgroup, setting, source))
    }

    fn write(
        &self,
        scheme: &str,
        subgroup: &str,
        setting: &str,
        source: PowerSource,
        value: u32,
    ) -> SysResult<()> {
        if lock(&self.fail_settings).iter().any(|s| s == setting) {
            return Err(SysError::new(
                SysErrorKind::AccessDenied,
                "write power setting",
                setting,
                "Access is denied.",
            ));
        }
        if !self.exists(scheme)? {
            return Err(Self::missing("write power setting", scheme));
        }
        lock(&self.settings).insert(
            (scheme.into(), subgroup.into(), setting.into(), source),
            value,
        );
        Ok(())
    }

    fn has_battery(&self) -> SysResult<bool> {
        Ok(*lock(&self.battery))
    }
}

/// A full fake system plus typed handles to each fake for assertions.
pub struct FakeSystem {
    pub ctx: SysCtx,
    pub registry: Arc<FakeRegistry>,
    pub services: Arc<FakeServices>,
    pub restore_points: Arc<FakeRestorePoints>,
    pub power: Arc<FakePower>,
}

impl FakeSystem {
    pub fn new() -> Self {
        let registry = FakeRegistry::new();
        let services = FakeServices::new();
        let restore_points = FakeRestorePoints::new(Some(true));
        let power = FakePower::new();
        let ctx = SysCtx {
            registry: registry.clone(),
            services: services.clone(),
            restore_points: restore_points.clone(),
            power: power.clone(),
        };
        Self {
            ctx,
            registry,
            services,
            restore_points,
            power,
        }
    }
}

impl Default for FakeSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_keys_are_created_and_only_empty_keys_deleted() {
        let reg = FakeRegistry::new();
        reg.set(Hive::LocalMachine, r"A\B\C", "v", &RegData::Dword(1))
            .unwrap();
        assert!(reg.has_key(Hive::LocalMachine, r"a\b"));
        assert!(!reg.delete_key_if_empty(Hive::LocalMachine, r"A\B").unwrap());
        reg.delete_value(Hive::LocalMachine, r"A\B\C", "V").unwrap();
        assert!(
            reg.delete_key_if_empty(Hive::LocalMachine, r"A\B\C")
                .unwrap()
        );
        assert!(reg.delete_key_if_empty(Hive::LocalMachine, r"A\B").unwrap());
        assert_eq!(
            reg.subkeys(Hive::LocalMachine, "A").unwrap(),
            Vec::<String>::new()
        );
    }
}
