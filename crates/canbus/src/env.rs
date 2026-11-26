//! Bus configuration from environment variables.

use canbus_core::{BusConfig, Error, Result, parse_bool};

/// Build a [`BusConfig`] from `CAN_*` environment variables.
///
/// | Variable | Meaning |
/// |---|---|
/// | `CAN_INTERFACE` | Interface name (required), e.g. `kvaser` |
/// | `CAN_CHANNEL` | Channel name (required), e.g. `0` or `PCAN_USBBUS1` |
/// | `CAN_BITRATE` | Nominal bitrate in bit/s |
/// | `CAN_DATA_BITRATE` | CAN FD data bitrate in bit/s (enables FD) |
/// | `CAN_FD` | `true` to require CAN FD (then `CAN_DATA_BITRATE` is required) |
/// | `CAN_LISTEN_ONLY` | `true` for listen-only mode |
///
/// The names match python-can's, so one environment configures both.
pub fn config_from_env() -> Result<BusConfig> {
    config_from_lookup(|key| std::env::var(key).ok())
}

/// Like [`config_from_env`], reading variables through `get` instead of the
/// process environment (useful for tests and config files).
pub fn config_from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<BusConfig> {
    let required = |key: &str| {
        get(key)
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| Error::InvalidConfig(format!("{key} is not set")))
    };
    let number = |key: &str| -> Result<Option<u32>> {
        get(key)
            .map(|v| {
                v.trim()
                    .parse::<u32>()
                    .map_err(|_| Error::InvalidConfig(format!("{key}: `{v}` is not a number")))
            })
            .transpose()
    };
    let flag = |key: &str| -> Result<Option<bool>> {
        get(key)
            .map(|v| {
                parse_bool(&v)
                    .ok_or_else(|| Error::InvalidConfig(format!("{key}: `{v}` is not a boolean")))
            })
            .transpose()
    };

    let mut config = BusConfig::new(
        required("CAN_INTERFACE")?.trim(),
        required("CAN_CHANNEL")?.trim(),
    );
    if let Some(bitrate) = number("CAN_BITRATE")? {
        config = config.bitrate(bitrate);
    }
    match (flag("CAN_FD")?, number("CAN_DATA_BITRATE")?) {
        (Some(false), Some(_)) => {
            return Err(Error::InvalidConfig(
                "CAN_DATA_BITRATE is set but CAN_FD is false".into(),
            ));
        }
        (Some(true), None) => {
            return Err(Error::InvalidConfig(
                "CAN_FD is true but CAN_DATA_BITRATE is not set".into(),
            ));
        }
        (_, Some(data_bitrate)) => config = config.fd(data_bitrate),
        (_, None) => {}
    }
    if let Some(listen_only) = flag("CAN_LISTEN_ONLY")? {
        config = config.listen_only(listen_only);
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lookup(vars: &[(&str, &str)]) -> Result<BusConfig> {
        let map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        config_from_lookup(|k| map.get(k).cloned())
    }

    #[test]
    fn full_config() {
        let cfg = lookup(&[
            ("CAN_INTERFACE", "pcan"),
            ("CAN_CHANNEL", "PCAN_USBBUS1"),
            ("CAN_BITRATE", "500000"),
            ("CAN_DATA_BITRATE", "2000000"),
            ("CAN_LISTEN_ONLY", "1"),
        ])
        .unwrap();
        assert_eq!(cfg.interface, "pcan");
        assert_eq!(cfg.channel, "PCAN_USBBUS1");
        assert_eq!(cfg.bitrate, Some(500_000));
        assert_eq!(cfg.fd.unwrap().data_bitrate, 2_000_000);
        assert!(cfg.listen_only);
    }

    #[test]
    fn interface_and_channel_are_required() {
        assert!(lookup(&[("CAN_CHANNEL", "0")]).is_err());
        assert!(lookup(&[("CAN_INTERFACE", "kvaser")]).is_err());
        let cfg = lookup(&[("CAN_INTERFACE", "kvaser"), ("CAN_CHANNEL", "0")]).unwrap();
        assert_eq!(cfg.bitrate, None);
        assert!(cfg.fd.is_none());
    }

    #[test]
    fn invalid_values_are_reported() {
        let base = [("CAN_INTERFACE", "virtual"), ("CAN_CHANNEL", "v")];
        let with = |extra: (&'static str, &'static str)| {
            let mut v = base.to_vec();
            v.push(extra);
            lookup(&v)
        };
        assert!(with(("CAN_BITRATE", "fast")).is_err());
        assert!(with(("CAN_LISTEN_ONLY", "maybe")).is_err());
        assert!(with(("CAN_FD", "true")).is_err());
        let mut both = base.to_vec();
        both.extend([("CAN_FD", "false"), ("CAN_DATA_BITRATE", "2000000")]);
        assert!(lookup(&both).is_err());
    }
}
