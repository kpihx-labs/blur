use std::{
    collections::BTreeMap,
    io::ErrorKind,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use if_addrs::{IfAddr, get_if_addrs};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    cli::{
        LanArgs, LanCommand, LanDiscoverArgs, LanHostArgs, LanJoinArgs, LanNetworkArgs, PlayArgs,
        RuntimeChoice,
    },
    profile, runtime,
};

const MAGIC: &str = "blur-lan";
const PROTOCOL: u16 = 1;
const ORIGINAL_LAN_PORT: u16 = 47801;
const MAX_PACKET: usize = 2048;
const CLOCK_WINDOW: Duration = Duration::from_secs(30);
const RESPONSE_RATE_LIMIT_WINDOW: Duration = Duration::from_millis(500);
const MAX_RATE_LIMIT_SOURCES: usize = 256;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum PacketKind {
    Query,
    Advertise,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LanPacket {
    pub magic: String,
    pub protocol: u16,
    pub kind: PacketKind,
    pub session: String,
    pub player: String,
    pub compatibility: String,
    pub game_port: u16,
    pub sent_unix: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredPeer {
    pub source: SocketAddrV4,
    pub packet: LanPacket,
    pub compatible: bool,
}

#[derive(Clone, Debug)]
struct Interface {
    name: String,
    ip: Ipv4Addr,
    broadcast: Ipv4Addr,
    netmask: Ipv4Addr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryPolicy {
    pub bind_ip: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub port: u16,
    pub allow_public: bool,
    pub clock_window: Duration,
    pub expected_compatibility: String,
}

pub fn run(args: LanArgs) -> Result<()> {
    match args.command {
        LanCommand::Doctor(args) => doctor(args),
        LanCommand::Discover(args) => print_discovery(args),
        LanCommand::Host(args) => host(args),
        LanCommand::Join(args) => join(args),
    }
}

pub fn compatibility_hash(game_sha256: &str, runtime_sha256: &str, input_sha256: &str) -> String {
    let material = format!("blur-lan-v{PROTOCOL}:{game_sha256}:{runtime_sha256}:{input_sha256}");
    format!("{:x}", Sha256::digest(material.as_bytes()))
}

pub fn encode_packet(packet: &LanPacket) -> Result<Vec<u8>> {
    validate_packet(packet)?;
    let encoded = serde_json::to_vec(packet)?;
    anyhow::ensure!(
        encoded.len() <= MAX_PACKET,
        "LAN packet exceeds {MAX_PACKET} bytes"
    );
    Ok(encoded)
}

pub fn decode_packet(bytes: &[u8]) -> Result<LanPacket> {
    anyhow::ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_PACKET,
        "invalid LAN packet size"
    );
    let packet: LanPacket = serde_json::from_slice(bytes).context("invalid LAN packet JSON")?;
    validate_packet(&packet)?;
    Ok(packet)
}

pub fn classify_packet(
    packet: LanPacket,
    source: SocketAddrV4,
    expected_compatibility: &str,
) -> DiscoveredPeer {
    let compatible = packet.compatibility == expected_compatibility;
    DiscoveredPeer {
        source,
        packet,
        compatible,
    }
}

pub fn filter_compatible(peers: Vec<DiscoveredPeer>) -> Vec<DiscoveredPeer> {
    peers.into_iter().filter(|peer| peer.compatible).collect()
}

fn doctor(args: LanNetworkArgs) -> Result<()> {
    let interface = select_interface(&args)?;
    println!(
        "ok   interface {}: {} -> {}",
        interface.name, interface.ip, interface.broadcast
    );
    let socket = bind_socket(&interface, args.port)?;
    println!("ok   discovery UDP {}:{}", interface.ip, args.port);
    drop(socket);
    println!("ok   original Blur LAN transport: UDP {ORIGINAL_LAN_PORT} (vendor-fixed)");
    println!(
        "note firewall rules were not inspected or modified; allow inbound UDP manually if needed"
    );
    println!("note LAN is same-subnet only: no cloud, account, relay, VPN, or port forwarding");
    Ok(())
}

fn print_discovery(args: LanDiscoverArgs) -> Result<()> {
    let peers = discover(&args)?;
    let mut shown = 0;
    for peer in &peers {
        if peer.compatible || args.show_incompatible {
            println!(
                "{}\t{}\t{}\t{}",
                peer.packet.session,
                peer.packet.player,
                peer.source.ip(),
                if peer.compatible {
                    "compatible"
                } else {
                    "incompatible"
                }
            );
            shown += 1;
        }
    }
    let rejected = peers.iter().filter(|peer| !peer.compatible).count();
    if shown == 0 {
        println!("no compatible Blur LAN sessions found");
    }
    if rejected > 0 && !args.show_incompatible {
        eprintln!("rejected {rejected} incompatible peer advertisement(s)");
    }
    Ok(())
}

fn host(args: LanHostArgs) -> Result<()> {
    validate_label("session", &args.session, 64)?;
    let profile = profile::load(&args.profile)?;
    let player = args.player.unwrap_or_else(|| profile.name.clone());
    validate_label("player", &player, 32)?;
    let interface = select_interface(&args.network)?;
    let socket = bind_socket(&interface, args.network.port)?;
    let compatibility = runtime::native_compatibility_fingerprint(&profile)?;
    let packet = advertisement(&args.session, &player, &compatibility)?;
    let policy = policy_for(&interface, &args.network, compatibility);
    let stop = Arc::new(AtomicBool::new(false));
    let handle = advertise(
        socket,
        interface.broadcast,
        args.network.port,
        packet,
        policy,
        Arc::clone(&stop),
    );

    println!(
        "advertising '{}' on {}:{}; original transport UDP {}",
        args.session, interface.ip, args.network.port, ORIGINAL_LAN_PORT
    );
    println!("firewall is never modified automatically");
    let play_args = PlayArgs {
        profile: profile.name.clone(),
        runtime: RuntimeChoice::Native,
        web_port: args.web_port,
        webgl2: false,
        no_open: args.no_open,
        lan: true,
    };
    let status = runtime::native(&profile, &play_args, Some(&player));
    stop.store(true, Ordering::Relaxed);
    handle
        .join()
        .map_err(|_| anyhow::anyhow!("LAN advertiser thread panicked"))??;
    let status = status?;
    anyhow::ensure!(status.success(), "game runtime exited with {status}");
    Ok(())
}

fn join(args: LanJoinArgs) -> Result<()> {
    let profile_name = args.discovery.profile.clone();
    let peers = discover(&args.discovery)?;
    let mut compatible = filter_compatible(peers)
        .into_iter()
        .filter(|peer| {
            args.session
                .as_ref()
                .is_none_or(|session| peer.packet.session == *session)
        })
        .collect::<Vec<_>>();
    compatible.sort_by(|left, right| left.packet.session.cmp(&right.packet.session));
    anyhow::ensure!(
        !compatible.is_empty(),
        "no requested compatible LAN session found"
    );
    anyhow::ensure!(
        compatible.len() == 1,
        "multiple compatible sessions found; select one with --session"
    );
    let selected = &compatible[0];
    println!(
        "compatibility preflight selected '{}' advertised by {} at {}",
        selected.packet.session,
        selected.packet.player,
        selected.source.ip()
    );
    println!(
        "the original game LAN UI remains authoritative over UDP {ORIGINAL_LAN_PORT}; discovery only preflights compatibility"
    );
    crate::runtime::play(PlayArgs {
        profile: profile_name,
        runtime: RuntimeChoice::Native,
        web_port: args.web_port,
        webgl2: false,
        no_open: args.no_open,
        lan: true,
    })
}

fn discover(args: &LanDiscoverArgs) -> Result<Vec<DiscoveredPeer>> {
    let profile = profile::load(&args.profile)?;
    let expected_compatibility = runtime::native_compatibility_fingerprint(&profile)?;
    let interface = select_interface(&args.network)?;
    let socket = bind_socket(&interface, args.network.port)?;
    socket.set_read_timeout(Some(Duration::from_millis(100)))?;
    let policy = policy_for(&interface, &args.network, expected_compatibility.clone());
    let query = LanPacket {
        magic: MAGIC.to_owned(),
        protocol: PROTOCOL,
        kind: PacketKind::Query,
        session: String::new(),
        player: String::new(),
        compatibility: expected_compatibility.clone(),
        game_port: ORIGINAL_LAN_PORT,
        sent_unix: now_unix()?,
    };
    socket.send_to(
        &encode_packet(&query)?,
        SocketAddrV4::new(interface.broadcast, args.network.port),
    )?;

    let deadline = Instant::now() + args.timeout;
    let mut buffer = [0_u8; MAX_PACKET];
    let mut peers = BTreeMap::new();
    while Instant::now() < deadline {
        match socket.recv_from(&mut buffer) {
            Ok((size, SocketAddr::V4(source))) => {
                if let Ok(packet) = decode_packet(&buffer[..size])
                    && packet.kind == PacketKind::Advertise
                    && validate_peer(&packet, source, &policy, now_unix()?).is_ok()
                {
                    let peer = classify_packet(packet, source, &expected_compatibility);
                    peers.insert(
                        (peer.source.ip().to_owned(), peer.packet.session.clone()),
                        peer,
                    );
                }
            }
            Ok(_) => {}
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(error) => return Err(error).context("LAN discovery receive failed"),
        }
    }
    Ok(peers.into_values().collect())
}

fn advertise(
    socket: UdpSocket,
    broadcast: Ipv4Addr,
    port: u16,
    packet: LanPacket,
    policy: DiscoveryPolicy,
    stop: Arc<AtomicBool>,
) -> thread::JoinHandle<Result<()>> {
    thread::spawn(move || {
        socket.set_read_timeout(Some(Duration::from_millis(250)))?;
        let destination = SocketAddrV4::new(broadcast, port);
        let mut last_broadcast = Instant::now() - Duration::from_secs(3);
        let mut recent_responses = BTreeMap::new();
        let mut buffer = [0_u8; MAX_PACKET];
        while !stop.load(Ordering::Relaxed) {
            if last_broadcast.elapsed() >= Duration::from_secs(2) {
                let encoded = encode_packet_with_timestamp(&packet, now_unix()?)?;
                socket.send_to(&encoded, destination)?;
                last_broadcast = Instant::now();
            }
            match socket.recv_from(&mut buffer) {
                Ok((size, SocketAddr::V4(source))) => {
                    if let Ok(query) = decode_packet(&buffer[..size])
                        && query.kind == PacketKind::Query
                        && query.compatibility == policy.expected_compatibility
                        && validate_peer(&query, source, &policy, now_unix()?).is_ok()
                        && permit_rate_limited_response(
                            &mut recent_responses,
                            source,
                            Instant::now(),
                        )
                    {
                        let encoded = encode_packet_with_timestamp(&packet, now_unix()?)?;
                        socket.send_to(&encoded, source)?;
                    }
                }
                Ok(_) => {}
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                Err(error) => return Err(error).context("LAN advertiser receive failed"),
            }
        }
        Ok(())
    })
}

fn encode_packet_with_timestamp(packet: &LanPacket, sent_unix: u64) -> Result<Vec<u8>> {
    let mut refreshed = packet.clone();
    refreshed.sent_unix = sent_unix;
    encode_packet(&refreshed)
}

fn permit_rate_limited_response(
    recent_responses: &mut BTreeMap<SocketAddrV4, Instant>,
    source: SocketAddrV4,
    now: Instant,
) -> bool {
    recent_responses.retain(|_, allowed_after| *allowed_after > now);
    if recent_responses.contains_key(&source) || recent_responses.len() >= MAX_RATE_LIMIT_SOURCES {
        return false;
    }
    recent_responses.insert(source, now + RESPONSE_RATE_LIMIT_WINDOW);
    true
}

fn advertisement(session: &str, player: &str, compatibility: &str) -> Result<LanPacket> {
    let packet = LanPacket {
        magic: MAGIC.to_owned(),
        protocol: PROTOCOL,
        kind: PacketKind::Advertise,
        session: session.to_owned(),
        player: player.to_owned(),
        compatibility: compatibility.to_owned(),
        game_port: ORIGINAL_LAN_PORT,
        sent_unix: now_unix()?,
    };
    validate_packet(&packet)?;
    Ok(packet)
}

fn select_interface(args: &LanNetworkArgs) -> Result<Interface> {
    let mut candidates = get_if_addrs()?
        .into_iter()
        .filter_map(|interface| match interface.addr {
            IfAddr::V4(address) => address.broadcast.map(|broadcast| Interface {
                name: interface.name,
                ip: address.ip,
                broadcast,
                netmask: address.netmask,
            }),
            IfAddr::V6(_) => None,
        })
        .collect::<Vec<_>>();

    if let Some(bind) = args.bind {
        anyhow::ensure!(
            bind.is_private() || args.allow_public,
            "refusing non-private interface {bind}; pass --allow-public only on a trusted network"
        );
        return candidates
            .into_iter()
            .find(|interface| interface.ip == bind)
            .with_context(|| format!("{bind} is not a local broadcast-capable IPv4 interface"));
    }

    candidates.retain(|interface| {
        interface.ip.is_private()
            && !interface.ip.is_loopback()
            && !is_virtual_interface(&interface.name)
    });
    candidates.sort_by(|left, right| left.name.cmp(&right.name).then(left.ip.cmp(&right.ip)));
    candidates.into_iter().next().context(
        "no private broadcast-capable IPv4 interface found; select one explicitly with --bind",
    )
}

fn is_virtual_interface(name: &str) -> bool {
    [
        "br-",
        "docker",
        "podman",
        "tailscale",
        "tun",
        "tap",
        "veth",
        "virbr",
        "vmnet",
        "wg",
        "zt",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
}

fn bind_socket(interface: &Interface, port: u16) -> Result<UdpSocket> {
    anyhow::ensure!(port != 0, "LAN discovery port cannot be zero");
    let socket = UdpSocket::bind(SocketAddrV4::new(interface.ip, port))
        .with_context(|| format!("could not bind UDP {}:{port}", interface.ip))?;
    socket.set_broadcast(true)?;
    Ok(socket)
}

fn policy_for(
    interface: &Interface,
    args: &LanNetworkArgs,
    expected_compatibility: String,
) -> DiscoveryPolicy {
    DiscoveryPolicy {
        bind_ip: interface.ip,
        netmask: interface.netmask,
        port: args.port,
        allow_public: args.allow_public,
        clock_window: CLOCK_WINDOW,
        expected_compatibility,
    }
}

pub fn validate_peer(
    packet: &LanPacket,
    source: SocketAddrV4,
    policy: &DiscoveryPolicy,
    now_unix: u64,
) -> Result<()> {
    validate_packet(packet)?;
    anyhow::ensure!(source.port() != 0, "invalid LAN discovery source port");
    anyhow::ensure!(
        source.port() == policy.port,
        "unexpected LAN discovery source port {}",
        source.port()
    );
    anyhow::ensure!(
        source.ip() != &Ipv4Addr::UNSPECIFIED
            && source.ip() != &Ipv4Addr::BROADCAST
            && !source.ip().is_multicast()
            && !source.ip().is_loopback(),
        "invalid LAN discovery source address {}",
        source.ip()
    );
    anyhow::ensure!(
        packet.sent_unix >= now_unix.saturating_sub(policy.clock_window.as_secs())
            && packet.sent_unix <= now_unix.saturating_add(policy.clock_window.as_secs()),
        "stale or future LAN packet timestamp"
    );
    if policy.allow_public {
        anyhow::ensure!(
            !source.ip().is_documentation(),
            "invalid public LAN discovery source {}",
            source.ip()
        );
    }
    anyhow::ensure!(
        same_subnet(*source.ip(), policy.bind_ip, policy.netmask),
        "LAN discovery source {} is outside the selected local subnet",
        source.ip()
    );
    if !policy.allow_public {
        anyhow::ensure!(
            source.ip().is_private(),
            "LAN discovery source {} is outside the selected local subnet",
            source.ip()
        );
    }
    Ok(())
}

fn validate_packet(packet: &LanPacket) -> Result<()> {
    anyhow::ensure!(packet.magic == MAGIC, "invalid LAN packet magic");
    anyhow::ensure!(packet.protocol == PROTOCOL, "unsupported LAN protocol");
    anyhow::ensure!(
        packet.game_port == ORIGINAL_LAN_PORT,
        "unsupported game LAN port"
    );
    anyhow::ensure!(
        packet.compatibility.len() == 64
            && packet
                .compatibility
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()),
        "invalid compatibility hash"
    );
    match packet.kind {
        PacketKind::Query => {
            anyhow::ensure!(
                packet.session.is_empty() && packet.player.is_empty(),
                "invalid query payload"
            );
        }
        PacketKind::Advertise => {
            validate_label("session", &packet.session, 64)?;
            validate_label("player", &packet.player, 32)?;
        }
    }
    Ok(())
}

fn validate_label(label: &str, value: &str, maximum: usize) -> Result<()> {
    anyhow::ensure!(
        !value.is_empty() && value.len() <= maximum,
        "{label} must be 1-{maximum} bytes"
    );
    anyhow::ensure!(
        value.chars().all(|character| !character.is_control()),
        "{label} contains control characters"
    );
    Ok(())
}

fn now_unix() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn same_subnet(left: Ipv4Addr, right: Ipv4Addr, netmask: Ipv4Addr) -> bool {
    let mask = u32::from(netmask);
    (u32::from(left) & mask) == (u32::from(right) & mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet() -> LanPacket {
        LanPacket {
            magic: MAGIC.to_owned(),
            protocol: PROTOCOL,
            kind: PacketKind::Advertise,
            session: "Session".to_owned(),
            player: "Player".to_owned(),
            compatibility: "a".repeat(64),
            game_port: ORIGINAL_LAN_PORT,
            sent_unix: 1,
        }
    }

    #[test]
    fn refreshed_packets_reencode_with_the_current_timestamp() {
        let packet = packet();
        let refreshed = decode_packet(&encode_packet_with_timestamp(&packet, 42).unwrap()).unwrap();
        assert_eq!(refreshed.sent_unix, 42);
        assert_eq!(refreshed.session, packet.session);
        assert_eq!(refreshed.player, packet.player);
        assert_eq!(refreshed.compatibility, packet.compatibility);
    }

    #[test]
    fn response_rate_limit_is_per_source_and_bounded() {
        let start = Instant::now();
        let source = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 10), 47802);
        let mut recent = BTreeMap::new();
        assert!(permit_rate_limited_response(&mut recent, source, start));
        assert!(!permit_rate_limited_response(
            &mut recent,
            source,
            start + Duration::from_millis(100)
        ));
        assert!(permit_rate_limited_response(
            &mut recent,
            source,
            start + RESPONSE_RATE_LIMIT_WINDOW
        ));

        let mut saturated = BTreeMap::new();
        for index in 0..MAX_RATE_LIMIT_SOURCES {
            let source = SocketAddrV4::new(
                Ipv4Addr::new(10, 0, (index / 254) as u8, ((index % 254) + 1) as u8),
                47802,
            );
            assert!(permit_rate_limited_response(&mut saturated, source, start));
        }
        let overflow = SocketAddrV4::new(Ipv4Addr::new(10, 0, 1, 1), 47802);
        assert!(!permit_rate_limited_response(
            &mut saturated,
            overflow,
            start
        ));
    }
}
