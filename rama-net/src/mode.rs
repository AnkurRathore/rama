#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Enum representing the IP modes that can be used by the DNS resolver.
pub enum DnsResolveIpMode {
    #[default]
    Dual,
    SingleIpV4,
    SingleIpV6,
    DualPreferIpV4
}

impl DnsResolveIpMode{
   
    /// checks if IPv4 is supported in current mode
    pub fn ipv4_supported(&self) -> bool {
        matches!(self.mode, DnsResolveIpMode::Dual | DnsResolveIpMode::SingleIpV4 | DnsResolveIpMode::DualPreferIpV4)
    }

    /// checks if IPv6 is supported in current mode
    pub fn ipv6_supported(&self) -> bool {
        matches!(self.mode, DnsResolveIpMode::Dual | DnsResolveIpMode::SingleIpV6)
    }
}

///Mode for establishing a connection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectIpMode {
    #[default]
    Dual,
    Ipv4,
    Ipv6,
}