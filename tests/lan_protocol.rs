use std::net::{Ipv4Addr, SocketAddrV4};

use blur::lan::{
    DiscoveryPolicy, LanPacket, PacketKind, classify_packet, compatibility_hash, decode_packet,
    encode_packet, filter_compatible, validate_peer,
};

fn advertisement(compatibility: String) -> LanPacket {
    LanPacket {
        magic: "blur-lan".to_owned(),
        protocol: 2,
        kind: PacketKind::Advertise,
        session: "Saturday race".to_owned(),
        player: "KpihX".to_owned(),
        compatibility,
        language: "en".to_owned(),
        game_port: 47801,
        sent_unix: 1,
    }
}

fn policy(expected_compatibility: &str) -> DiscoveryPolicy {
    DiscoveryPolicy {
        bind_ip: Ipv4Addr::new(192, 168, 1, 10),
        netmask: Ipv4Addr::new(255, 255, 255, 0),
        port: 47802,
        allow_public: false,
        clock_window: std::time::Duration::from_secs(30),
        expected_compatibility: expected_compatibility.to_owned(),
        expected_language: "en".to_owned(),
    }
}

#[test]
fn lan_packet_round_trips_and_rejects_malformed_or_wrong_protocol_data() {
    let packet = advertisement(compatibility_hash(
        &"a".repeat(64),
        &"b".repeat(64),
        &"c".repeat(64),
        &"d".repeat(64),
    ));
    let encoded = encode_packet(&packet).unwrap();
    assert_eq!(decode_packet(&encoded).unwrap(), packet);
    assert!(decode_packet(b"not-json").is_err());

    let mut wrong_protocol = packet;
    wrong_protocol.protocol = 99;
    assert!(encode_packet(&wrong_protocol).is_err());

    let non_hex = advertisement("z".repeat(64));
    assert!(encode_packet(&non_hex).is_err());
}

#[test]
fn discovery_filter_rejects_incompatible_peers() {
    let source = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 20), 47802);
    let expected = compatibility_hash(
        &"a".repeat(64),
        &"b".repeat(64),
        &"c".repeat(64),
        &"d".repeat(64),
    );
    let accepted = classify_packet(advertisement(expected.clone()), source, &expected, "en");
    let rejected = classify_packet(advertisement("0".repeat(64)), source, &expected, "en");
    let filtered = filter_compatible(vec![accepted.clone(), rejected.clone()]);
    assert_eq!(filtered, vec![accepted]);
    assert!(!rejected.compatible);
}

#[test]
fn discovery_validation_accepts_same_subnet_private_sources_only() {
    let expected = compatibility_hash(
        &"a".repeat(64),
        &"b".repeat(64),
        &"c".repeat(64),
        &"d".repeat(64),
    );
    let packet = advertisement(expected.clone());
    let source = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 20), 47802);
    assert!(validate_peer(&packet, source, &policy(&expected), 1).is_ok());

    let wrong_subnet = SocketAddrV4::new(Ipv4Addr::new(192, 168, 2, 20), 47802);
    assert!(validate_peer(&packet, wrong_subnet, &policy(&expected), 1).is_err());

    let wrong_port = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 20), 53000);
    assert!(validate_peer(&packet, wrong_port, &policy(&expected), 1).is_err());
}

#[test]
fn discovery_validation_allows_public_bind_only_for_same_subnet_sources() {
    let expected = compatibility_hash(
        &"a".repeat(64),
        &"b".repeat(64),
        &"c".repeat(64),
        &"d".repeat(64),
    );
    let packet = advertisement(expected.clone());
    let policy = DiscoveryPolicy {
        bind_ip: Ipv4Addr::new(100, 64, 0, 10),
        netmask: Ipv4Addr::new(255, 255, 255, 0),
        port: 47802,
        allow_public: true,
        clock_window: std::time::Duration::from_secs(30),
        expected_compatibility: expected,
        expected_language: "en".to_owned(),
    };

    let same_subnet = SocketAddrV4::new(Ipv4Addr::new(100, 64, 0, 42), 47802);
    assert!(validate_peer(&packet, same_subnet, &policy, 1).is_ok());

    let off_subnet = SocketAddrV4::new(Ipv4Addr::new(100, 64, 1, 42), 47802);
    assert!(validate_peer(&packet, off_subnet, &policy, 1).is_err());
}

#[test]
fn discovery_validation_rejects_stale_and_future_packets() {
    let expected = compatibility_hash(
        &"a".repeat(64),
        &"b".repeat(64),
        &"c".repeat(64),
        &"d".repeat(64),
    );
    let source = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 20), 47802);

    let mut stale = advertisement(expected.clone());
    stale.sent_unix = 1;
    assert!(validate_peer(&stale, source, &policy(&expected), 100).is_err());

    let mut future = advertisement(expected);
    future.sent_unix = 200;
    assert!(validate_peer(&future, source, &policy(&future.compatibility), 100).is_err());
}

#[test]
fn discovery_validation_rejects_language_field_fingerprint_mismatch() {
    let expected = compatibility_hash(
        &"a".repeat(64),
        &"b".repeat(64),
        &"c".repeat(64),
        &"d".repeat(64),
    );
    let mut packet = advertisement(expected.clone());
    packet.language = "fr".to_owned();
    let source = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 20), 47802);
    assert!(validate_peer(&packet, source, &policy(&expected), 1).is_ok());
    assert!(!classify_packet(packet, source, &expected, "en").compatible);
}

#[test]
fn protocol_v1_packets_decode_but_are_never_v2_compatible() {
    let encoded = format!(
        r#"{{"magic":"blur-lan","protocol":1,"kind":"advertise","session":"Legacy","player":"Peer","compatibility":"{}","game_port":47801,"sent_unix":1}}"#,
        "a".repeat(64)
    );
    let packet = decode_packet(encoded.as_bytes()).unwrap();
    assert_eq!(packet.protocol, 1);
    assert!(packet.language.is_empty());
    let source = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 20), 47802);
    assert!(validate_peer(&packet, source, &policy(&"b".repeat(64)), 1).is_ok());
    assert!(!classify_packet(packet, source, &"a".repeat(64), "en").compatible);
}

#[test]
fn malformed_packet_limits_and_canonical_fields_are_enforced() {
    assert!(decode_packet(&vec![b' '; 2049]).is_err());

    let mut packet = advertisement("A".repeat(64));
    assert!(encode_packet(&packet).is_err());

    packet.compatibility = "a".repeat(64);
    packet.language = "fr-fr".to_owned();
    assert!(encode_packet(&packet).is_err());
    packet.language = "fr-FR".to_owned();
    assert!(encode_packet(&packet).is_ok());

    packet.session = "x".repeat(65);
    assert!(encode_packet(&packet).is_err());
}
