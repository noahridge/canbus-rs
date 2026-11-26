//! The [`Bus`] facade.

use core::fmt;
use core::time::Duration;

use canbus_core::{
    blocking_receive, blocking_transmit, nb_receive, nb_transmit, BusConfig, BusStatus, CanBus,
    CanMessage, Capabilities, Deadline, EmbeddedCanError, Error, Filter, FilterSupport, Interface,
    Result,
};

/// An open CAN bus on any backend.
///
/// `Bus` wraps the backend's [`CanBus`] and adds what every application
/// wants regardless of hardware:
///
/// - opening by interface name ([`Bus::open`]) or from the environment
///   ([`Bus::from_env`]);
/// - software filtering when the backend can't enforce filters exactly
///   (see [`FilterSupport::Software`]);
/// - `embedded-can` trait impls.
///
/// `Bus` implements [`CanBus`] itself, so it can be used anywhere a backend
/// can.
pub struct Bus {
    inner: Box<dyn CanBus>,
    interface: &'static str,
    filters: Vec<Filter>,
    software_filter: bool,
}

impl Bus {
    /// Open the interface named in `config.interface`.
    ///
    /// Fails with [`Error::UnknownInterface`] if that interface isn't
    /// compiled in (see [`crate::interfaces`]).
    pub fn open(config: &BusConfig) -> Result<Self> {
        let interface = crate::interface(&config.interface)
            .ok_or_else(|| Error::UnknownInterface(config.interface.clone()))?;
        Self::open_with(interface, config)
    }

    /// Open a bus on a specific interface, e.g. one from a third-party crate.
    pub fn open_with(interface: &Interface, config: &BusConfig) -> Result<Self> {
        Ok(Self::wrap(interface.name, interface.open(config)?))
    }

    /// Open the bus described by the `CAN_*` environment variables.
    ///
    /// See [`crate::config_from_env`].
    pub fn from_env() -> Result<Self> {
        Self::open(&crate::config_from_env()?)
    }

    /// Wrap a backend bus that was opened directly.
    pub fn from_backend(interface: &'static str, bus: impl CanBus + 'static) -> Self {
        Self::wrap(interface, Box::new(bus))
    }

    fn wrap(interface: &'static str, inner: Box<dyn CanBus>) -> Self {
        Self {
            inner,
            interface,
            filters: Vec::new(),
            software_filter: false,
        }
    }

    /// Name of the interface this bus was opened on.
    pub fn interface(&self) -> &'static str {
        self.interface
    }

    /// The filters currently installed with [`CanBus::set_filters`].
    pub fn filters(&self) -> &[Filter] {
        &self.filters
    }

    /// Unwrap the backend bus. Software filtering stops applying.
    pub fn into_inner(self) -> Box<dyn CanBus> {
        self.inner
    }
}

impl fmt::Debug for Bus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bus")
            .field("interface", &self.interface)
            .field("channel", &self.inner.channel_info())
            .field("filters", &self.filters)
            .field("software_filter", &self.software_filter)
            .finish()
    }
}

impl CanBus for Bus {
    fn send(&mut self, msg: &CanMessage, timeout: Option<Duration>) -> Result<()> {
        self.inner.send(msg, timeout)
    }

    fn recv(&mut self, timeout: Option<Duration>) -> Result<Option<CanMessage>> {
        if !self.software_filter {
            return self.inner.recv(timeout);
        }
        let deadline = Deadline::after(timeout);
        loop {
            match self.inner.recv(deadline.remaining())? {
                Some(msg) if Filter::any_matches(&self.filters, &msg) => return Ok(Some(msg)),
                Some(_) if deadline.expired() => return Ok(None),
                Some(_) => continue,
                None => return Ok(None),
            }
        }
    }

    fn set_filters(&mut self, filters: &[Filter]) -> Result<FilterSupport> {
        let support = match self.inner.set_filters(filters) {
            Ok(support) => support,
            // A backend without filter support still gets correct results.
            Err(Error::Unsupported(_)) => FilterSupport::Software,
            Err(e) => return Err(e),
        };
        self.filters = filters.to_vec();
        self.software_filter = support == FilterSupport::Software && !filters.is_empty();
        // Every frame `recv` returns now matches, either way.
        Ok(FilterSupport::Hardware)
    }

    fn status(&self) -> Result<BusStatus> {
        self.inner.status()
    }

    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }

    fn channel_info(&self) -> &str {
        self.inner.channel_info()
    }

    fn flush_tx(&mut self) -> Result<()> {
        self.inner.flush_tx()
    }

    fn shutdown(&mut self) -> Result<()> {
        self.inner.shutdown()
    }
}

impl embedded_can::nb::Can for Bus {
    type Frame = CanMessage;
    type Error = EmbeddedCanError;

    fn transmit(&mut self, frame: &CanMessage) -> nb::Result<Option<CanMessage>, Self::Error> {
        nb_transmit(self, frame)
    }

    fn receive(&mut self) -> nb::Result<CanMessage, Self::Error> {
        nb_receive(self)
    }
}

impl embedded_can::blocking::Can for Bus {
    type Frame = CanMessage;
    type Error = EmbeddedCanError;

    fn transmit(&mut self, frame: &CanMessage) -> core::result::Result<(), Self::Error> {
        blocking_transmit(self, frame)
    }

    fn receive(&mut self) -> core::result::Result<CanMessage, Self::Error> {
        blocking_receive(self)
    }
}
