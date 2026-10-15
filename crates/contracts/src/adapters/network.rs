//! S16 network and remote-access adapters.
//!
//! Checked against the collectors' own emit declarations: `plugins/powershell-adapter/domains/network.ps1`
//! for nearly all of this category, `plugins/powershell-adapter/collect-system.ps1` for
//! `network.adapters` and `network.addresses`, `domains/scoped.ps1` for `apps.mihomo`, and
//! `collect-domain.ps1`'s `Registry`/`Registry-Key` helpers for the shape every registry item has.
//! The payload keys below are those emitters' own key names.
//!
//! Six identity shapes occur in this one category and they are not interchangeable, so each source
//! takes the shape its type declares rather than one applied uniformly:
//!
//!   * **`native_identifier`** — `network.adapter`, `network.connection_profile`,
//!     `network.firewall_profile`, `network.ipsec_rule`, `network.vpn_configuration`. The native
//!     identifier is the item id the collector keyed the object by: the interface GUID, the profile
//!     name, the firewall profile's name, the IPsec rule's name, the VPN connection's name.
//!   * **`native_path`** — `network.remote_access_configuration`. The registry key's own path, which
//!     is also the record's identity within the source.
//!   * **`named_entity`** — `network.smb_share`. The share's name.
//!   * **`composite_native`** — `network.address_configuration`, `network.route`,
//!     `network.dns_configuration`, `network.dns_cache_entry`, `network.hosts_entry`,
//!     `network.firewall_rule`, `network.smb_mapping`, `network.smb_session`. Several payload keys
//!     together; a component the source does not supply is *listed as missing*, never filled with a
//!     neighbouring value.
//!   * **`value_key`** — `network.proxy_configuration`. One candidate per entry in a registry key's
//!     value table, keyed by the key's path and the value's name (`proxy_configuration` below).
//!   * **`endpoint_tuple`** — `network.endpoint`. The seven-part endpoint tuple, which the TCP source
//!     fills from a connection and the UDP source fills only partly from a local listener.
//!
//! Four things this category deliberately does not do:
//!
//!   * **fill in an identity component the material does not carry.** Five sources here state less
//!     than their type's key asks for, and the identity says so instead of substituting: the
//!     interface GUID is in no payload of `network.addresses`, `network.routes` or
//!     `network.dns-configuration` (each names its interface by index and alias, which are not the
//!     GUID); `Get-SmbSession` names a session's client and user and never its server; and the UDP
//!     endpoint table carries no remote pair and no protocol component. Those are reported as
//!     `missing`, and the identity engine decides what a provisional identity means — a value chosen
//!     here would be a wrong identity, not an approximate one.
//!   * **read a snapshot as a lifecycle fact.** `network.dns-cache` states its own coverage — "cache
//!     snapshot; not query time or complete DNS traffic" — and `network.tcp`/`network.udp` are
//!     point-in-time enumerations of live endpoints. An endpoint missing from a later snapshot is not
//!     an endpoint that ended, and nothing here concludes one did.
//!   * **state an action.** `network.dns_cache_entry`, `network.endpoint` and `network.smb_share`
//!     declare actions, and `model/source-map.cjs` assigns every source in this module the
//!     `observation` destination: the actions those types carry come from the event lanes, not from
//!     these snapshots. Nothing here turns a cached record into a query, an open port into a
//!     connection, or a share's ACL into an access.
//!   * **map a payload stream no declared type describes.** Every stream these sources emit belongs
//!     to a type declared for the source, so nothing is skipped here; the nearest case is
//!     `network.dns-configuration`'s machine-wide search key, whose identity components the material
//!     cannot supply and whose `search_list` field the type does declare (see
//!     `dns_search_configuration`).
//!
//! **On a field the source could not read.** `unreadable` appears here only where a registry item's
//! `values` is present but is not a value table — the collector was looking at the key and what it
//! read is not the configuration. No emitter in this category otherwise marks a single field as
//! unread, and a key an emitter did not write is simply absent rather than unreadable.
//!
//! **On the checked-in fixtures.** `fixtures/01-baseline.jsonl` carries a `pcscope.network.tcp` and a
//! `pcscope.network.firewall-rules` item whose keys are the *catalog's field ids* (`local_address`,
//! `direction`, …) rather than the emitter's own (`LocalAddress`, `Direction`, …). They predate the
//! collectors and no example reads them; this module reads what the emitter writes, and the
//! difference is for S21's key-name reconciliation.
use super::*;

pub static SOURCES: &[SourceSpec] = &[
    SourceSpec { source: "pcscope.network.adapters", type_id: Some("network.adapter"), build: adapters },
    SourceSpec { source: "pcscope.network.addresses", type_id: Some("network.address_configuration"), build: addresses },
    // Three item shapes and three declared types, so the shape decides the destination.
    SourceSpec { source: "pcscope.network.access", type_id: None, build: access },
    SourceSpec { source: "pcscope.network.dns-cache", type_id: Some("network.dns_cache_entry"), build: dns_cache },
    // Two item shapes, one declared type.
    SourceSpec { source: "pcscope.network.dns-configuration", type_id: Some("network.dns_configuration"), build: dns_configuration },
    SourceSpec { source: "pcscope.network.firewall-profiles", type_id: Some("network.firewall_profile"), build: firewall_profiles },
    SourceSpec { source: "pcscope.network.firewall-rules", type_id: Some("network.firewall_rule"), build: firewall_rules },
    SourceSpec { source: "pcscope.network.hosts", type_id: Some("network.hosts_entry"), build: hosts },
    SourceSpec { source: "pcscope.network.ipsec", type_id: Some("network.ipsec_rule"), build: ipsec_rules },
    SourceSpec { source: "pcscope.network.mapped-drives", type_id: Some("network.smb_mapping"), build: mapped_drives },
    SourceSpec { source: "pcscope.network.profiles", type_id: Some("network.connection_profile"), build: profiles },
    SourceSpec { source: "pcscope.network.routes", type_id: Some("network.route"), build: routes },
    SourceSpec { source: "pcscope.network.share-sessions", type_id: Some("network.smb_session"), build: share_sessions },
    SourceSpec { source: "pcscope.network.shares", type_id: Some("network.smb_share"), build: shares },
    SourceSpec { source: "pcscope.network.tcp", type_id: Some("network.endpoint"), build: tcp_endpoints },
    SourceSpec { source: "pcscope.network.udp", type_id: Some("network.endpoint"), build: udp_endpoints },
    SourceSpec { source: "pcscope.network.winrm-configuration", type_id: Some("network.remote_access_configuration"), build: winrm_configuration },
    SourceSpec { source: "pcscope.apps.mihomo", type_id: Some("network.proxy_configuration"), build: mihomo },
];

// ---------- shared shapes ----------

/// A field whose value the adapter states because the source it adapts is the thing that makes it
/// true, not because a payload key happened to carry it.
fn known(field_id: &str, kind: FieldKind, value: Value) -> FieldCandidate {
    FieldCandidate {
        field_id: field_id.into(),
        field_kind: kind,
        value_type: value_type(&value),
        value: Some(value),
        status: ValueStatus::Known,
        source_time: None,
        derivation: Derivation::SourceProvided,
        conflicts: Vec::new(),
    }
}

/// A field this candidate's own material does not state, for the places where a declared field has
/// no key to read at all and `field()` would be handed a key that never exists.
fn not_provided(field_id: &str, kind: FieldKind) -> FieldCandidate {
    FieldCandidate {
        field_id: field_id.into(),
        field_kind: kind,
        value: None,
        value_type: "unknown".into(),
        status: ValueStatus::NotProvided,
        source_time: None,
        derivation: Derivation::SourceProvided,
        conflicts: Vec::new(),
    }
}

/// The value table of a registry item: `name -> {kind,value}`, which is what `Registry-Key` gives
/// every registry item it writes and what no other item shape in this category has.
fn values_of(payload: &Value) -> Option<&serde_json::Map<String, Value>> {
    payload.get("values").and_then(Value::as_object)
}

/// Whether the item is one of the registry-shaped payloads (`{path;hive;view;values;basis;collection}`).
fn registry_shaped(payload: &Value) -> bool {
    values_of(payload).is_some()
}

/// The whole value table as one field, for the types that declare a single configuration field
/// rather than one field per value. A `values` key that is there but is not a table is `unreadable`:
/// the collector was looking at the key, and what it read is not the configuration.
fn values_field(payload: &Value, field_id: &str) -> FieldCandidate {
    match payload.get("values") {
        Some(Value::Object(values)) => known(field_id, FieldKind::Configuration, Value::Object(values.clone())),
        Some(_) => unreadable(field_id, FieldKind::Configuration),
        None => field(payload, field_id, "values", FieldKind::Configuration),
    }
}

/// The scope a registry item's objects belong to.
///
/// The current user's hive is the one case in this category that is genuinely per-user, so it keeps
/// the scope it was handed, collecting-account segment included: two users' Internet Settings are two
/// configurations and must never be merged by name. A machine hive belongs to the installation — the
/// account the collection ran as is *how* the key was read, not *what* it configures — so the
/// machine-level narrowing `installation_scope` performs is applied.
fn hive_scope(context: &AdapterContext, payload: &Value) -> String {
    match text(payload, "hive").filter(|hive| hive.eq_ignore_ascii_case("CurrentUser")) {
        Some(_) => context.collected_scope.trim().trim_end_matches('/').to_owned(),
        None => installation_scope(&context.collected_scope),
    }
}

/// The scope a value-level identity is formed in: the hive's scope plus the registry view.
///
/// `network.proxy_configuration` declares `[scope, path, value_name]` and has no key for the view,
/// yet `Registry-Key` reads the same path under `Registry64` and `Registry32`. The view therefore
/// goes into the one component the declared keys leave room for, which is the convention
/// `adapter-source-survey.md` §3.5 records and `policy.rs` implements as `value_scope`. Types whose
/// keys already have a `hive` or a `view` of their own do not do this and do not need it.
fn value_scope(context: &AdapterContext, payload: &Value) -> String {
    let base = hive_scope(context, payload);
    match text(payload, "view").filter(|view| !view.trim().is_empty()) {
        Some(view) => format!("{base}/view:{view}"),
        None => base,
    }
}

/// The last non-empty segment of a registry path — the key's own name. The remote-access type's scope
/// is the install scope plus a service name, and the name of the key the collector read is the
/// source's own statement of which service it configures.
fn key_name_of(path: &str) -> Option<String> {
    path.split('\\').filter(|segment| !segment.trim().is_empty()).next_back().map(str::to_owned)
}

// ---------- pcscope.network.adapters ----------

/// `Get-NetAdapter -IncludeHidden`, keyed by the interface GUID the collector built the item id from.
///
/// The collector writes six named properties, and none of them is the GUID: the item id *is* the
/// GUID, so that is where the native identifier and the `guid` field are read from. `-IncludeHidden`
/// says hidden adapters were collected, not that this adapter is hidden, and the class's own `Hidden`
/// property is not among the keys the emitter writes — so `hidden` is absent rather than filled with
/// the meaning of the switch.
fn adapters(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let guid = input.item_id.clone();
    let identity = identity("network.adapter", &scope, &input.evidence.id, &[("native_id", Some(guid.clone()))]);
    let fields = vec![
        known("guid", FieldKind::Identity, Value::String(guid)),
        field(&input.payload, "name", "name", FieldKind::Configuration),
        field(&input.payload, "status", "status", FieldKind::State),
        field(&input.payload, "mac", "mac_address", FieldKind::Configuration),
        field(&input.payload, "speed", "link_speed", FieldKind::State),
        field(&input.payload, "driver_version", "driver_version", FieldKind::Configuration),
        field(&input.payload, "hidden", "hidden", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.network.addresses ----------

/// `Get-NetIPAddress`, one item per interface, address family and address.
///
/// The type's composite asks for the interface **GUID**, and this emitter does not state it: it names
/// the interface by `interface_index` and `interface_alias`. Neither is the GUID, so the component is
/// listed as missing rather than filled with either — the same treatment the interface's own `family`
/// gets only from the address family the item carries. The `interface` field, which is not an
/// identity key, is read from the alias, because that is the source's own name for the interface.
fn addresses(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let identity = identity(
        "network.address_configuration",
        &scope,
        &input.evidence.id,
        &[
            // The emitter states no interface GUID; the index it does state is a different value.
            ("interface_guid", None),
            ("address", text(&input.payload, "address")),
            ("prefix_length", text(&input.payload, "prefix_length")),
        ],
    );
    let fields = vec![
        field(&input.payload, "interface", "interface_alias", FieldKind::Identity),
        field(&input.payload, "address", "address", FieldKind::Identity),
        field(&input.payload, "prefix_length", "prefix_length", FieldKind::Identity),
        field(&input.payload, "family", "address_family", FieldKind::Configuration),
        // `address_state` is what the address is doing now, not how it is configured.
        field(&input.payload, "state", "address_state", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.network.routes ----------

/// `Get-NetRoute`, one item per interface, destination prefix, next hop and policy store.
///
/// The same absent component as `addresses`: the type keys a route by the interface GUID and the
/// emitter names the interface by index and alias. The route's destination prefix and next hop are
/// carried as the emitted them, and `RouteMetric` and `PolicyStore` are the metric and policy source
/// the type declares. `Publish`, `Protocol` and `AddressFamily` are collected and no declared field
/// reads them, so they stay in the material.
fn routes(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let identity = identity(
        "network.route",
        &scope,
        &input.evidence.id,
        &[
            ("interface_guid", None),
            ("destination_prefix", text(&input.payload, "DestinationPrefix")),
            ("next_hop", text(&input.payload, "NextHop")),
        ],
    );
    let fields = vec![
        field(&input.payload, "interface", "InterfaceAlias", FieldKind::Identity),
        field(&input.payload, "destination", "DestinationPrefix", FieldKind::Identity),
        field(&input.payload, "next_hop", "NextHop", FieldKind::Identity),
        field(&input.payload, "metric", "RouteMetric", FieldKind::Configuration),
        field(&input.payload, "policy_source", "PolicyStore", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.network.profiles ----------

/// `Get-NetConnectionProfile`, keyed by the item the collector built from the interface index and the
/// network name.
///
/// The category is configuration and the two connectivity columns are runtime state, so they are
/// mapped as state: a network that is currently unreachable has not stopped being a Private network.
fn profiles(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let identity = identity("network.connection_profile", &scope, &input.evidence.id, &[("native_id", Some(input.item_id.clone()))]);
    let fields = vec![
        field(&input.payload, "name", "Name", FieldKind::Identity),
        field(&input.payload, "category", "NetworkCategory", FieldKind::Configuration),
        field(&input.payload, "ipv4_connectivity", "IPv4Connectivity", FieldKind::State),
        field(&input.payload, "ipv6_connectivity", "IPv6Connectivity", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.network.dns-configuration ----------

/// `Get-DnsClientServerAddress` and the machine's `Tcpip\Parameters` key: two item shapes, one
/// declared type.
///
/// The shape decides the branch, as it does for `policy.system.clock`. The interface list is keyed by
/// the interface and the address family, and again the emitter names the interface by index and alias
/// rather than by GUID, so that component is listed as missing. The registry key is the machine-wide
/// part of the configuration and states no interface at all — `search_list` is what it carries — so
/// both of the type's components are missing there rather than borrowed from the interface list.
fn dns_configuration(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if registry_shaped(&input.payload) {
        return dns_search_configuration(input, context);
    }
    let scope = installation_scope(&context.collected_scope);
    let identity = identity(
        "network.dns_configuration",
        &scope,
        &input.evidence.id,
        &[("interface_guid", None), ("address_family", text(&input.payload, "AddressFamily"))],
    );
    let fields = vec![
        field(&input.payload, "interface", "InterfaceAlias", FieldKind::Identity),
        field(&input.payload, "servers", "ServerAddresses", FieldKind::Configuration),
        // This shape states no search list; that is the registry key's.
        not_provided("search_list", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

/// `SYSTEM\CurrentControlSet\Services\Tcpip\Parameters`: the machine's DNS domain and search list.
///
/// The key is one record and its path is not one of this type's identity keys — the type keys a DNS
/// configuration by interface and address family, neither of which a machine-wide key states. Both
/// components are therefore listed as missing rather than filled with the registry path or a made-up
/// family, and the record is provisional on exactly that. `search_list` is read from the value the
/// key holds under that name; `Domain`, `NV Domain` and `UseDomainNameDevolution` are collected and
/// the type declares no field for them, so they stay in the material.
fn dns_search_configuration(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    // The key is read only from the machine hive, so the install scope is what it belongs to.
    let scope = installation_scope(&context.collected_scope);
    let identity = identity("network.dns_configuration", &scope, &input.evidence.id, &[("interface_guid", None), ("address_family", None)]);
    let fields = vec![
        field(&input.payload, "path", "path", FieldKind::Configuration),
        // A machine-wide key names no interface.
        not_provided("interface", FieldKind::Identity),
        // It states DNS servers for no interface, so there are none to report here.
        not_provided("servers", FieldKind::Configuration),
        registry_field(&input.payload, "search_list", "SearchList", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.network.dns-cache ----------

/// `Get-DnsClientCache`, keyed by the record's name and type.
///
/// The collector states the coverage of this material itself — `basis: "cache snapshot; not query
/// time or complete DNS traffic"` — and that statement travels with it. The type declares an action
/// (`network.dns_query`) and no action is emitted from it: a cached record is not a query, and a
/// record that is gone from a later snapshot expired rather than being deleted by a user, which the
/// type's own scope says. `entry` (the name that was looked up), `status`, `section` and `basis` are
/// collected and no declared field reads them, so they stay in the material.
fn dns_cache(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let identity = identity(
        "network.dns_cache_entry",
        &scope,
        &input.evidence.id,
        &[("name", text(&input.payload, "name")), ("record_type", text(&input.payload, "type"))],
    );
    let fields = vec![
        field(&input.payload, "name", "name", FieldKind::Identity),
        field(&input.payload, "record_type", "type", FieldKind::Identity),
        // The resolved value is what the record currently says, so it is state.
        field(&input.payload, "data", "data", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.network.hosts ----------

/// The hosts file, one item per name the collector parsed.
///
/// The emitter already splits a line into one item per name, and the item id is the line number and
/// that name, so an item is about exactly one name: the composite's `names` component carries the
/// name this item states, and an address with several names arrives as several items rather than as
/// one object the adapter would have to assemble. Comments were removed by the collector before the
/// item was built, and a file outside the safe read policy or a line it could not parse fails the
/// whole source rather than producing an item — so an item the adapter sees was read. `line` and
/// `basis` stay in the material.
fn hosts(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let identity = identity(
        "network.hosts_entry",
        &scope,
        &input.evidence.id,
        &[("address", text(&input.payload, "address")), ("names", text(&input.payload, "name"))],
    );
    let fields = vec![
        field(&input.payload, "address", "address", FieldKind::Identity),
        field(&input.payload, "names", "name", FieldKind::Identity),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.network.tcp and pcscope.network.udp ----------

/// The protocol the TCP source enumerates.
///
/// Neither emitter writes a `Protocol` payload key: `Get-NetTCPConnection` is the TCP table and
/// `Get-NetUDPEndpoint` the UDP table, so the protocol is what the source is rather than a value one
/// of its keys carried — the same kind of statement as `software.rs` naming the npm manager for a
/// material that says only `name` and `version`. The TCP material is a connection: it states both
/// ends, and the protocol is therefore the component that completes its tuple. The UDP material is a
/// local listening endpoint with no remote side, and it states no protocol component either; its
/// identity lists what is missing instead of holding a value no key carried.
const TCP_PROTOCOL: &str = "TCP";

/// One endpoint from one item, for either protocol's table.
///
/// Everything except the protocol comes from the item's own keys, and a key the emitter did not write
/// is listed as missing. The two sources are snapshots of live endpoints, so neither this adapter nor
/// anything downstream may read a later snapshot's silence as an endpoint having ended.
fn endpoint(input: &EvidenceInput, context: &AdapterContext, protocol: Option<&str>) -> AdapterResult {
    let payload = &input.payload;
    let scope = installation_scope(&context.collected_scope);
    let identity = identity(
        "network.endpoint",
        &scope,
        &input.evidence.id,
        &[
            ("protocol", protocol.map(str::to_owned)),
            ("local_address", text(payload, "LocalAddress")),
            ("local_port", text(payload, "LocalPort")),
            ("remote_address", text(payload, "RemoteAddress")),
            ("remote_port", text(payload, "RemotePort")),
            ("owning_pid", text(payload, "OwningProcess")),
            ("created_at", text(payload, "CreationTime")),
        ],
    );
    let fields = vec![
        match protocol {
            Some(protocol) => known("protocol", FieldKind::Configuration, Value::String(protocol.to_owned())),
            // The UDP emitter writes no protocol key; none is assumed for it either.
            None => not_provided("protocol", FieldKind::Configuration),
        },
        field(payload, "local_address", "LocalAddress", FieldKind::Identity),
        field(payload, "local_port", "LocalPort", FieldKind::Identity),
        field(payload, "remote_address", "RemoteAddress", FieldKind::Identity),
        field(payload, "remote_port", "RemotePort", FieldKind::Identity),
        field(payload, "state", "State", FieldKind::State),
        field(payload, "owning_pid", "OwningProcess", FieldKind::Configuration),
        // The same instant the tuple's `created_at` component carries, under the field's own name.
        field(payload, "creation_time", "CreationTime", FieldKind::Identity),
    ];
    object_candidate(input, context, identity, fields)
}

/// `Get-NetTCPConnection`: the tuple's seven components are all supplied, six of them by the item.
fn tcp_endpoints(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    endpoint(input, context, Some(TCP_PROTOCOL))
}

/// `Get-NetUDPEndpoint`: a local listening endpoint, with no remote address, no remote port and no
/// protocol component in its material. Those three are listed as missing, which is what makes the
/// identity provisional rather than wrong.
fn udp_endpoints(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    endpoint(input, context, None)
}

// ---------- pcscope.network.access ----------

/// The Internet Settings key and the Terminal Server key are the registry shapes; a third item shape
/// is a VPN connection.
fn is_proxy_key(payload: &Value) -> bool {
    text(payload, "path").map(|path| path.to_lowercase().contains("internet settings")).unwrap_or(false)
}

/// `pcscope.network.access` emits three shapes and the source maps to three declared types, so the
/// shape decides the destination: a registry key under the current user's Internet Settings is a
/// proxy configuration, the Terminal Server key is remote-access configuration, and an item carrying
/// a connection name is a VPN connection. A shape none of the three describes fails with the reason
/// visible rather than being filed under whichever type comes first.
fn access(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    if registry_shaped(payload) {
        return if is_proxy_key(payload) {
            proxy_configuration(input, context)
        } else {
            remote_access_key(input, context)
        };
    }
    if text(payload, "Name").is_some() {
        return vpn_connection(input, context);
    }
    AdapterResult::failed("network access item carries neither a registry value table nor a VPN connection name", false)
}

/// The declared field a proxy value's own name states, when the type has one for it.
///
/// `network.proxy_configuration` declares three settings — whether the proxy is enabled, the proxy
/// server and the bypass list — and a registry key holds those as three named values. `registry_field`
/// reads a declared field from a value name it is given; here the candidate *is* a value name, so
/// deciding which declared field it stands for is the same reading, with the name doing the saying.
fn proxy_field(name: &str) -> Option<&'static str> {
    match name.to_ascii_lowercase().as_str() {
        "proxyenable" => Some("enabled"),
        "proxyserver" => Some("server"),
        "proxyoverride" => Some("bypass"),
        _ => None,
    }
}

/// `Software\Microsoft\Windows\CurrentVersion\Internet Settings` under the current user's hive.
///
/// Value-level, like the policy sources: the type's identity is `[scope, path, value_name]`, so the
/// key expands into one candidate per entry in its value table. `Registry-Key`'s `$allowed` parameter
/// is not a filter — it keeps configuration values rather than a display allowlist — so `ProxyServer`
/// is collected even though the emitter's list does not name it, and a name the key does not hold
/// produces no object at all rather than an object with a defaulted value.
///
/// The scope carries the hive and the registry view: the current user's hive is per-user and keeps
/// the scope it was handed, so two users' Internet Settings cannot merge, and the view goes into the
/// scope because the declared keys have no component for it.
fn proxy_configuration(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    let Some(values) = values_of(payload) else {
        return AdapterResult::failed("proxy configuration item carries no value table; a value-level identity cannot be formed", false);
    };
    let scope = value_scope(context, payload);
    let path = text(payload, "path");
    let hive = text(payload, "hive");
    let candidates: Vec<ObservationCandidate> = values
        .keys()
        .map(|name| {
            let own = proxy_field(name);
            let mut fields = vec![
                match hive.clone() {
                    Some(hive) => known("scope", FieldKind::Identity, Value::String(hive)),
                    None => field(payload, "scope", "hive", FieldKind::Identity),
                },
                field(payload, "path", "path", FieldKind::Identity),
            ];
            // Every declared setting is reported for every candidate, and only the candidate that
            // *is* that value states one: a value that holds no proxy setting is an entry the type
            // declares no field for, and it stays one rather than borrowing its neighbours' data.
            // The value is read by the name the table actually holds, so a key that spells it in
            // another case is still the same setting.
            for field_id in ["enabled", "server", "bypass"] {
                fields.push(if own == Some(field_id) {
                    registry_field(payload, field_id, name, FieldKind::Configuration)
                } else {
                    not_provided(field_id, FieldKind::Configuration)
                });
            }
            ObservationCandidate {
                identity: identity("network.proxy_configuration", &scope, &input.evidence.id, &[("path", path.clone()), ("value_name", Some(name.clone()))]),
                fields,
                observed_at: input.observed_at.clone(),
                evidence: vec![input.evidence.clone()],
                complete: context.complete,
            }
        })
        .collect();
    observations(candidates)
}

/// The Terminal Server key (`network.access`) and the WSMAN key (`network.winrm-configuration`): one
/// registry key per record, keyed by the key's own path, which is `native_path`.
///
/// The `service` field is the key's own name — the last segment of the path the collector read — which
/// is the source's statement of which service the key configures. The whole value table is carried as
/// the type's single `values` field.
///
/// The type's declared keys have no component for the registry view, which `Registry` reads the key
/// under twice; that is noted here rather than repaired by putting a view into an identity the
/// catalog does not declare one in.
fn remote_access_key(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    let Some(path) = text(payload, "path").filter(|_| registry_shaped(payload)) else {
        return AdapterResult::failed("remote access item carries no registry path; its identity cannot be formed", false);
    };
    let scope = installation_scope(&context.collected_scope);
    let identity = identity("network.remote_access_configuration", &scope, &input.evidence.id, &[("path", Some(path.clone()))]);
    let fields = vec![
        match key_name_of(&path) {
            Some(service) => known("service", FieldKind::Identity, Value::String(service)),
            None => not_provided("service", FieldKind::Identity),
        },
        field(payload, "path", "path", FieldKind::Identity),
        values_field(payload, "values"),
    ];
    object_candidate(input, context, identity, fields)
}

/// A `Get-VpnConnection` item, keyed by the connection's name.
///
/// The type's scope is the current user's, so the scope handed in is kept: the connections this
/// emitter returns are the collecting account's, and `AllUserConnection` — which the emitter also
/// writes — stays in the material because no declared field names it. What the type does declare and
/// this material does not carry is the server: `Get-VpnConnection` has a `ServerAddress` property and
/// the emitter writes six other keys, so the field is absent rather than rebuilt from the name.
fn vpn_connection(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = context.collected_scope.trim().to_owned();
    let identity = identity("network.vpn_configuration", &scope, &input.evidence.id, &[("native_id", Some(input.item_id.clone()))]);
    let fields = vec![
        field(&input.payload, "name", "Name", FieldKind::Identity),
        field(&input.payload, "server", "ServerAddress", FieldKind::Configuration),
        field(&input.payload, "tunnel_type", "TunnelType", FieldKind::Configuration),
        field(&input.payload, "status", "ConnectionStatus", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.network.firewall-profiles ----------

/// `Get-NetFirewallProfile`, one item per profile, keyed by the profile's name.
///
/// `inbound` and `outbound` are the profile's default actions, `enabled` its switch and `log_path` the
/// file it logs to. The remaining switches the emitter collects — the three `Allow*Rules` flags, the
/// `Log*` flags and the size limit — are configuration this type declares no field for, so they stay
/// in the material rather than being folded into `enabled`.
fn firewall_profiles(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let identity = identity("network.firewall_profile", &scope, &input.evidence.id, &[("native_id", Some(input.item_id.clone()))]);
    let fields = vec![
        field(&input.payload, "name", "Name", FieldKind::Identity),
        field(&input.payload, "inbound", "DefaultInboundAction", FieldKind::Configuration),
        field(&input.payload, "outbound", "DefaultOutboundAction", FieldKind::Configuration),
        field(&input.payload, "enabled", "Enabled", FieldKind::Configuration),
        field(&input.payload, "log_path", "LogFileName", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.network.firewall-rules ----------

/// The policy store the firewall and IPsec emitters read from.
///
/// It is not a payload key: it is the `-PolicyStore ActiveStore` argument in the collector's own
/// command, and the rule type's scope names exactly that store. Stating it is the adapter saying
/// which source it adapts, the same kind of statement as `software.rs` naming npm and `startup.rs`
/// naming the WMI namespace — not a value read out of the item.
const ACTIVE_STORE: &str = "ActiveStore";

/// `Get-NetFirewallRule -PolicyStore ActiveStore`, keyed by the store and the rule's name.
///
/// The emitter joins seven filter tables to each rule by `InstanceID` and *fails the whole source* if
/// one association is missing, so a rule that reaches this adapter always had all seven; the
/// association gap is reported by the source itself, as a gap, and no per-item state is invented for
/// it here. The `filters` table is collected and the type declares no field for it, so it stays in
/// the material. The rule's own `InstanceID` is the join key and is not among the keys the emitter
/// writes, so the declared `instance_id` field is absent rather than reconstructed from a filter's
/// key. `Metadata` and the two status columns are likewise unmapped.
fn firewall_rules(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let identity = identity(
        "network.firewall_rule",
        &scope,
        &input.evidence.id,
        &[("policy_store", Some(ACTIVE_STORE.to_owned())), ("name", text(&input.payload, "Name"))],
    );
    let fields = vec![
        field(&input.payload, "name", "Name", FieldKind::Identity),
        field(&input.payload, "direction", "Direction", FieldKind::Configuration),
        field(&input.payload, "action", "Action", FieldKind::Configuration),
        field(&input.payload, "enabled", "Enabled", FieldKind::Configuration),
        field(&input.payload, "profile", "Profile", FieldKind::Configuration),
        field(&input.payload, "instance_id", "InstanceID", FieldKind::Identity),
        field(&input.payload, "display_name", "DisplayName", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.network.ipsec ----------

/// `Get-NetIPsecRule -PolicyStore ActiveStore`, keyed by the rule's name.
///
/// The type's scope is the policy store's and its identity keys are `[scope, native_id]`, which leaves
/// no key for the store — so the store the emitter read from is folded into the scope, the same reason
/// the scope carries the registry view for the value-level types above and the same treatment S12
/// gives the registry view. Without it, the same rule name in two stores would be one instance.
///
/// The type declares a security mode and the emitter's own `Mode` key is the rule's mode (transport or
/// tunnel), which is what it is mapped from. `InboundSecurity` and `OutboundSecurity` state the
/// protection level each direction requires, and the type names no field for them, so neither is used
/// for `security_mode`. `policy_source` is the emitter's `PolicyStoreSource` — where the rule came
/// from, not the store it was read from.
fn ipsec_rules(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = format!("{}/policy-store:{}", installation_scope(&context.collected_scope), ACTIVE_STORE);
    let identity = identity("network.ipsec_rule", &scope, &input.evidence.id, &[("native_id", Some(input.item_id.clone()))]);
    let fields = vec![
        field(&input.payload, "name", "Name", FieldKind::Identity),
        field(&input.payload, "security_mode", "Mode", FieldKind::Configuration),
        field(&input.payload, "policy_source", "PolicyStoreSource", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.network.shares ----------

/// `Get-SmbShare` with `Get-SmbShareAccess` per share, keyed by the share's name.
///
/// `access` is the list of per-account share permissions the collector read, which is exactly the
/// share-level ACL the type declares — the source's own note is that this is the share's permission
/// list and not the filesystem's effective permissions. The list is carried whole; the type declares
/// one aggregate field for it, so it is not expanded into separate objects here. `description` and
/// `encrypt_data` are collected and no declared field reads them.
fn shares(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let name = text(&input.payload, "name").unwrap_or_else(|| input.item_id.clone());
    let identity = identity("network.smb_share", &scope, &input.evidence.id, &[("name", Some(name))]);
    let fields = vec![
        field(&input.payload, "name", "name", FieldKind::Identity),
        field(&input.payload, "path", "path", FieldKind::Configuration),
        field(&input.payload, "share_acl", "access", FieldKind::Configuration),
        field(&input.payload, "special", "special", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.network.share-sessions ----------

/// `Get-SmbSession`, one item per session the machine holds.
///
/// The type's composite is client, user and server, and `Get-SmbSession` states the first two and
/// never the third: these are the sessions *this* machine serves, and its own name is nowhere in the
/// material. The component is therefore listed as missing rather than filled with the collection
/// scope's machine name, which would be context dressed as a value — and it is one component the
/// source cannot supply for any session, not an unlucky item.
///
/// `Dialect` is the SMB dialect the session negotiated, which is the protocol the type declares.
/// `NumOpens` and `Encrypted` are runtime state. `SessionId` is what the collector keyed the item by
/// and the type's identity has no component for it.
fn share_sessions(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let identity = identity(
        "network.smb_session",
        &scope,
        &input.evidence.id,
        &[
            ("client", text(&input.payload, "ClientComputerName")),
            ("user", text(&input.payload, "ClientUserName")),
            // Get-SmbSession names the client and the user, never the server.
            ("server", None),
        ],
    );
    let fields = vec![
        field(&input.payload, "client", "ClientComputerName", FieldKind::Identity),
        field(&input.payload, "user", "ClientUserName", FieldKind::Identity),
        not_provided("server", FieldKind::Configuration),
        field(&input.payload, "open_count", "NumOpens", FieldKind::State),
        field(&input.payload, "protocol", "Dialect", FieldKind::Configuration),
        field(&input.payload, "encrypted", "Encrypted", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.network.mapped-drives ----------

/// `Get-SmbMapping`, keyed by the local name and the remote path.
///
/// The type's scope is the user's — a drive mapping belongs to the account it was made under — so the
/// scope handed in is kept, collecting-account segment included, rather than narrowed to the
/// installation. Two accounts mapping the same letter to different shares are two mappings.
fn mapped_drives(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = context.collected_scope.trim().to_owned();
    let identity = identity(
        "network.smb_mapping",
        &scope,
        &input.evidence.id,
        &[("local_name", text(&input.payload, "LocalPath")), ("remote_path", text(&input.payload, "RemotePath"))],
    );
    let fields = vec![
        field(&input.payload, "local_name", "LocalPath", FieldKind::Identity),
        field(&input.payload, "remote_path", "RemotePath", FieldKind::Identity),
        field(&input.payload, "status", "Status", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.network.winrm-configuration ----------

/// `SOFTWARE\Microsoft\Windows\CurrentVersion\WSMAN`, swept three levels deep.
///
/// Every item is a registry key read from the machine hive, so one key is one record keyed by its own
/// path. A non-registry item is not a shape this source emits, and says so. The source states that it
/// reads the configuration and does not connect to a remote machine, and nothing here reports a
/// remote session.
fn winrm_configuration(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    if !registry_shaped(&input.payload) {
        return AdapterResult::not_an_object("winrm configuration item is not a registry key; this source emits none");
    }
    remote_access_key(input, context)
}

// ---------- pcscope.apps.mihomo ----------

/// The Mihomo runtime configuration, read from its own loopback API when it is explicitly enabled.
///
/// The type is value-level — a proxy configuration is a value under a configuration location — and
/// this material states neither: it is one classification item, `runtime`, whose payload is a flat
/// table of the running proxy's mode, ports and switches. The item's own classification id is
/// therefore the location, the way `policy.rs` reads a literal item's id as its natural path, and the
/// value name is listed as missing rather than filled with one of the settings the payload happens to
/// hold.
///
/// None of the type's three settings is mapped: the material states no proxy enable flag, no proxy
/// server and no bypass list. What it states — the mode, the log level, the listening ports, whether
/// TUN is on — is a running instance's configuration, and the type declares no field for any of it.
/// The source's own limit is that it reads the local API only, with no subscription, node credential
/// or traffic content.
fn mihomo(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let scope = installation_scope(&context.collected_scope);
    let path = input.item_id.clone();
    let identity = identity(
        "network.proxy_configuration",
        &scope,
        &input.evidence.id,
        &[("path", Some(path.clone())), ("value_name", None)],
    );
    let fields = vec![
        // The settings are the running proxy's, not a hive's; no hive is stated here and none is
        // assumed.
        field(&input.payload, "scope", "hive", FieldKind::Identity),
        known("path", FieldKind::Identity, Value::String(path)),
        not_provided("enabled", FieldKind::Configuration),
        not_provided("server", FieldKind::Configuration),
        not_provided("bypass", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::EvidenceKind;
    use serde_json::json;

    fn input(source: &str, item_id: &str, payload: Value) -> EvidenceInput {
        EvidenceInput {
            source: source.into(),
            item_id: item_id.into(),
            structure_version: "v1".into(),
            source_scope: "machine:M/user:S-1-5-18/schema:1/context:/shard:0".into(),
            evidence: EvidenceRef { kind: EvidenceKind::Snapshot, id: format!("{source}|scope|run-1") },
            native_record_id: item_id.into(),
            source_time: None,
            observed_at: "2026-09-12T00:00:00Z".into(),
            ingested_at: "2026-09-12T00:00:01Z".into(),
            decoding: Decoding { available: true, ..Default::default() },
            payload,
        }
    }

    fn context_for(scope: &str) -> AdapterContext {
        AdapterContext {
            catalog_version: crate::model::catalog_version().into(),
            rule_version: crate::model::catalog_fingerprint(),
            machine_id: "M".into(),
            installation_id: "I".into(),
            boot_id: None,
            collected_scope: crate::semantics::business_scope(scope),
            complete: true,
        }
    }

    fn context() -> AdapterContext {
        context_for("machine:M/user:S-1-5-18/schema:1/context:/shard:0")
    }

    fn key_of(candidate: &IdentityCandidate) -> String {
        let mut parts: Vec<String> = candidate.fields.iter().map(|field| format!("{}={}", field.key, field.value)).collect();
        parts.sort();
        parts.join("|")
    }

    fn field_of<'a>(result: &'a AdapterResult, field_id: &str) -> &'a FieldCandidate {
        result.observations[0].fields.iter().find(|field| field.field_id == field_id).expect("field is mapped")
    }

    fn identity_field<'a>(result: &'a AdapterResult, key: &str) -> &'a IdentityField {
        result.observations[0].identity.fields.iter().find(|field| field.key == key).expect("identity key is supplied")
    }

    /// A registry item exactly as `Registry-Key` builds it.
    fn registry_item(hive: &str, view: &str, path: &str, values: Value) -> Value {
        json!({"path": path, "hive": hive, "view": view, "values": values, "basis": "configuration snapshot", "collection": {"policy": "evidence-v2"}})
    }

    /// One entry of `Registry-Key`'s value table.
    fn value(kind: &str, value: impl Into<Value>) -> Value {
        let value: Value = value.into();
        json!({"kind": kind, "value": value})
    }

    #[test]
    fn an_adapter_is_keyed_by_the_guid_the_collector_keyed_it_by() {
        let payload = json!({
            "name": "Ethernet",
            "description": "Intel(R) Ethernet Connection",
            "status": "Up",
            "mac_address": "00-11-22-33-44-55",
            "link_speed": "1 Gbps",
            "driver_version": "1.0.0.1"
        });
        let result = adapters(&input("pcscope.network.adapters", "{11111111-2222-3333-4444-555555555555}", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.adapter");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert_eq!(identity_field(&result, "native_id").value, "{11111111-2222-3333-4444-555555555555}");
        assert_eq!(field_of(&result, "guid").value, Some(json!("{11111111-2222-3333-4444-555555555555}")));
        assert_eq!(field_of(&result, "status").field_kind, FieldKind::State);
        // The source says hidden adapters were *collected*; it says nothing about this adapter, so the
        // field is absent rather than filled with the switch's meaning.
        assert_eq!(field_of(&result, "hidden").status, ValueStatus::NotProvided);
        // A machine-level object keeps the installation scope.
        assert_eq!(identity.scope, "machine:M");
    }

    #[test]
    fn an_address_states_no_interface_guid_and_is_not_keyed_by_its_index() {
        let payload = json!({
            "interface_index": 7,
            "interface_alias": "Ethernet",
            "address": "192.0.2.10",
            "prefix_length": 24,
            "address_family": "IPv4",
            "prefix_origin": "Manual",
            "suffix_origin": "Manual",
            "address_state": "Preferred"
        });
        let result = addresses(&input("pcscope.network.addresses", "7/IPv4/192.0.2.10", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.address_configuration");
        // The type keys the configuration by the interface GUID; the material states an index and an
        // alias, and neither is the GUID, so the component is listed missing rather than substituted.
        assert_eq!(identity.missing, vec!["interface_guid".to_owned()]);
        assert!(!identity.fields.iter().any(|field| field.key == "interface_guid"));
        assert_eq!(identity_field(&result, "address").value, "192.0.2.10");
        assert_eq!(identity_field(&result, "prefix_length").value, "24");
        assert_eq!(field_of(&result, "interface").value, Some(json!("Ethernet")), "the alias is the source's own name for the interface");
        assert_eq!(field_of(&result, "state").field_kind, FieldKind::State);
    }

    #[test]
    fn a_route_states_no_interface_guid_either_and_keeps_its_metric() {
        let payload = json!({
            "InterfaceIndex": 7,
            "InterfaceAlias": "Ethernet",
            "DestinationPrefix": "0.0.0.0/0",
            "NextHop": "192.0.2.1",
            "RouteMetric": 25,
            "Protocol": "NetMgmt",
            "AddressFamily": "IPv4",
            "PolicyStore": "ActiveStore",
            "Publish": "No"
        });
        let result = routes(&input("pcscope.network.routes", "7/0.0.0.0/0/192.0.2.1/ActiveStore", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.route");
        assert_eq!(identity.missing, vec!["interface_guid".to_owned()]);
        assert_eq!(identity_field(&result, "destination_prefix").value, "0.0.0.0/0");
        assert_eq!(identity_field(&result, "next_hop").value, "192.0.2.1");
        assert_eq!(field_of(&result, "metric").value, Some(json!(25)));
        assert_eq!(field_of(&result, "policy_source").value, Some(json!("ActiveStore")));
    }

    #[test]
    fn dns_configuration_maps_a_per_interface_list_and_the_machine_search_key_to_one_type() {
        let source = "pcscope.network.dns-configuration";
        let list = json!({
            "InterfaceIndex": 7,
            "InterfaceAlias": "Ethernet",
            "AddressFamily": "IPv4",
            "ServerAddresses": ["192.0.2.53", "198.51.100.53"]
        });
        let result = dns_configuration(&input(source, "7/IPv4", list), &context());
        assert_eq!(result.observations[0].identity.type_id, "network.dns_configuration");
        assert_eq!(result.observations[0].identity.missing, vec!["interface_guid".to_owned()]);
        assert_eq!(identity_field(&result, "address_family").value, "IPv4");
        assert_eq!(field_of(&result, "servers").value, Some(json!(["192.0.2.53", "198.51.100.53"])));
        assert_eq!(field_of(&result, "search_list").status, ValueStatus::NotProvided);

        // The machine-wide key is the same type's other shape: no interface, no address family, and a
        // search list of its own.
        let key = registry_item(
            "LocalMachine",
            "Registry64",
            "SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters",
            json!({"Domain": value("String", "example.test"), "SearchList": value("String", "example.test,corp.example.test")}),
        );
        let result = dns_configuration(&input(source, "LocalMachine/Registry64/parameters", key), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.dns_configuration");
        // Neither component can be supplied by a machine-wide key, and neither is invented.
        assert_eq!(identity.missing, vec!["interface_guid".to_owned(), "address_family".to_owned()]);
        assert_eq!(field_of(&result, "search_list").value, Some(json!("example.test,corp.example.test")));
        assert_eq!(field_of(&result, "servers").status, ValueStatus::NotProvided);
    }

    #[test]
    fn a_connection_profile_separates_its_category_from_its_connectivity() {
        let payload = json!({
            "InterfaceIndex": 7,
            "Name": "example-net",
            "NetworkCategory": "Private",
            "IPv4Connectivity": "Internet",
            "IPv6Connectivity": "NoTraffic"
        });
        let result = profiles(&input("pcscope.network.profiles", "7/example-net", payload), &context());
        assert!(result.observations[0].identity.missing.is_empty());
        assert_eq!(identity_field(&result, "native_id").value, "7/example-net");
        assert_eq!(field_of(&result, "category").value, Some(json!("Private")));
        assert_eq!(field_of(&result, "ipv4_connectivity").field_kind, FieldKind::State);
        assert_eq!(field_of(&result, "ipv6_connectivity").value, Some(json!("NoTraffic")));
    }

    #[test]
    fn a_tcp_endpoint_is_the_full_tuple_the_connection_states() {
        let payload = json!({
            "LocalAddress": "127.0.0.1",
            "LocalPort": 12345,
            "RemoteAddress": "192.0.2.9",
            "RemotePort": 443,
            "State": "Established",
            "OwningProcess": 4100,
            "CreationTime": "2026-09-12T00:00:00.0000000Z",
            "AppliedSetting": "Internet"
        });
        let result = tcp_endpoints(&input("pcscope.network.tcp", "127.0.0.1/12345/192.0.2.9/443/4100/2026-09-12T00:00:00Z", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.endpoint");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert_eq!(identity_field(&result, "protocol").value, "TCP");
        assert_eq!(identity_field(&result, "local_port").value, "12345");
        assert_eq!(identity_field(&result, "remote_address").value, "192.0.2.9");
        assert_eq!(identity_field(&result, "owning_pid").value, "4100");
        assert_eq!(identity_field(&result, "created_at").value, "2026-09-12T00:00:00.0000000Z");
        assert_eq!(field_of(&result, "state").value, Some(json!("Established")));
        assert_eq!(field_of(&result, "protocol").value, Some(json!("TCP")));
        // A snapshot of a live endpoint states no start and no end.
        assert!(result.actions.is_empty());
    }

    #[test]
    fn a_udp_endpoint_lists_the_three_components_its_emitter_does_not_supply() {
        let payload = json!({
            "LocalAddress": "0.0.0.0",
            "LocalPort": 5353,
            "OwningProcess": 1234,
            "CreationTime": "2026-09-12T00:00:00.0000000Z"
        });
        let result = udp_endpoints(&input("pcscope.network.udp", "0.0.0.0/5353/1234/2026-09-12T00:00:00Z", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.endpoint");
        // The endpoint table states a local listening endpoint only: no protocol component, no remote
        // address, no remote port. None of the three is guessed.
        assert_eq!(identity.missing, vec!["protocol".to_owned(), "remote_address".to_owned(), "remote_port".to_owned()]);
        assert!(!identity.fields.iter().any(|field| ["protocol", "remote_address", "remote_port"].contains(&field.key.as_str())));
        assert_eq!(identity_field(&result, "local_address").value, "0.0.0.0");
        assert_eq!(field_of(&result, "protocol").status, ValueStatus::NotProvided);
        assert_eq!(field_of(&result, "remote_address").status, ValueStatus::NotProvided);
        assert_eq!(field_of(&result, "local_port").value, Some(json!(5353)));
    }

    #[test]
    fn a_dns_cache_entry_is_a_snapshot_and_not_a_query() {
        let payload = json!({
            "entry": "example.test",
            "name": "example.test",
            "type": "A",
            "data": "192.0.2.10",
            "status": "Success",
            "section": "Answer",
            "basis": "cache snapshot; not query time or complete DNS traffic"
        });
        let result = dns_cache(&input("pcscope.network.dns-cache", "example.test/example.test/A/192.0.2.10", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.dns_cache_entry");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert_eq!(identity_field(&result, "name").value, "example.test");
        assert_eq!(identity_field(&result, "record_type").value, "A");
        assert_eq!(field_of(&result, "data").field_kind, FieldKind::State);
        // The type declares a query action; a cached record is not a query and none is emitted.
        assert!(result.actions.is_empty());
    }

    #[test]
    fn a_hosts_entry_carries_the_address_and_the_name_it_is_about() {
        let payload = json!({"address": "192.0.2.10", "name": "example.test", "line": 12, "basis": "hosts configuration; comments excluded"});
        let result = hosts(&input("pcscope.network.hosts", "12/example.test", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.hosts_entry");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert_eq!(identity_field(&result, "address").value, "192.0.2.10");
        assert_eq!(identity_field(&result, "names").value, "example.test");
        assert_eq!(field_of(&result, "names").value, Some(json!("example.test")));

        // A line with several names arrives as several items; each is keyed by the name it states.
        let other = hosts(&input("pcscope.network.hosts", "12/www.example.test", json!({"address": "192.0.2.10", "name": "www.example.test", "line": 12})), &context());
        assert_ne!(key_of(&identity), key_of(&other.observations[0].identity));
    }

    #[test]
    fn network_access_routes_its_three_item_shapes_to_three_types() {
        let source = "pcscope.network.access";
        let proxy = registry_item(
            "CurrentUser",
            "Registry64",
            "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
            json!({"ProxyEnable": value("DWord", 1)}),
        );
        let result = access(&input(source, "CurrentUser/Registry64/internet settings", proxy), &context());
        assert_eq!(result.observations[0].identity.type_id, "network.proxy_configuration");

        let terminal_server = registry_item(
            "LocalMachine",
            "Registry64",
            "SYSTEM\\CurrentControlSet\\Control\\Terminal Server",
            json!({"fDenyTSConnections": value("DWord", 1)}),
        );
        let result = access(&input(source, "LocalMachine/Registry64/terminal server", terminal_server), &context());
        assert_eq!(result.observations[0].identity.type_id, "network.remote_access_configuration");

        let vpn = json!({
            "Name": "Example VPN",
            "TunnelType": "Ikev2",
            "ConnectionStatus": "Disconnected",
            "SplitTunneling": false,
            "AllUserConnection": false,
            "AuthenticationMethod": "MachineCertificate"
        });
        let result = access(&input(source, "Example VPN", vpn), &context());
        assert_eq!(result.observations[0].identity.type_id, "network.vpn_configuration");

        // A shape none of the three describes says so instead of being filed under one of them.
        let result = access(&input(source, "empty", json!({"unrelated": true})), &context());
        assert!(matches!(result.outcome, AdapterOutcome::Failed { .. }), "{:?}", result.outcome);
    }

    #[test]
    fn a_proxy_key_expands_into_one_candidate_per_value_and_each_states_its_own_setting() {
        let payload = registry_item(
            "CurrentUser",
            "Registry64",
            "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
            json!({
                "ProxyEnable": value("DWord", 1),
                "ProxyServer": value("String", "192.0.2.9:8080"),
                "ProxyOverride": value("String", "<local>"),
                "AutoDetect": value("DWord", 0)
            }),
        );
        let result = access(&input("pcscope.network.access", "CurrentUser/Registry64/internet settings", payload.clone()), &context());
        assert_eq!(result.observations.len(), 4, "one value name is one object");
        for candidate in &result.observations {
            assert_eq!(candidate.identity.type_id, "network.proxy_configuration");
            assert!(candidate.identity.missing.is_empty(), "{:?}", candidate.identity.missing);
            assert!(candidate.identity.fields.iter().any(|field| field.key == "value_name"));
        }
        // The value name is the field's name for the three settings the type declares.
        let of_value = |name: &str| {
            result
                .observations
                .iter()
                .find(|candidate| candidate.identity.fields.iter().any(|field| field.key == "value_name" && field.value == name))
                .expect("a candidate per value")
                .clone()
        };
        let enabled = of_value("ProxyEnable");
        assert_eq!(enabled.fields.iter().find(|f| f.field_id == "enabled").expect("enabled").value, Some(json!(1)));
        assert_eq!(enabled.fields.iter().find(|f| f.field_id == "server").expect("server").status, ValueStatus::NotProvided);
        let server = of_value("ProxyServer");
        assert_eq!(server.fields.iter().find(|f| f.field_id == "server").expect("server").value, Some(json!("192.0.2.9:8080")));
        let bypass = of_value("ProxyOverride");
        assert_eq!(bypass.fields.iter().find(|f| f.field_id == "bypass").expect("bypass").value, Some(json!("<local>")));
        // A value the type names no setting for is still an object, and none of its neighbours' data
        // is copied onto it.
        let other = of_value("AutoDetect");
        for field_id in ["enabled", "server", "bypass"] {
            assert_eq!(other.fields.iter().find(|f| f.field_id == field_id).expect("declared field").status, ValueStatus::NotProvided);
        }

        // The current user's hive keeps the account it was read for, and the view is in the scope
        // because the declared identity keys have no component for it.
        let first = key_of(&result.observations[0].identity);
        assert!(result.observations[0].identity.scope.starts_with("machine:M/user:S-1-5-18/view:Registry64"), "{}", result.observations[0].identity.scope);
        let other_user = context_for("machine:M/user:S-1-5-21-9/schema:1/context:/shard:0");
        let result = access(&input("pcscope.network.access", "CurrentUser/Registry64/internet settings", payload), &other_user);
        assert_ne!(first, key_of(&result.observations[0].identity), "two users' settings are two configurations");
    }

    #[test]
    fn a_terminal_server_key_is_one_remote_access_record_named_by_its_key() {
        let payload = registry_item(
            "LocalMachine",
            "Registry64",
            "SYSTEM\\CurrentControlSet\\Control\\Terminal Server",
            json!({"fDenyTSConnections": value("DWord", 1)}),
        );
        let result = access(&input("pcscope.network.access", "LocalMachine/Registry64/terminal server", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.remote_access_configuration");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert_eq!(identity_field(&result, "path").value, "SYSTEM\\CurrentControlSet\\Control\\Terminal Server");
        assert_eq!(field_of(&result, "service").value, Some(json!("Terminal Server")));
        assert_eq!(field_of(&result, "values").value.as_ref().unwrap()["fDenyTSConnections"]["value"], json!(1));
        assert_eq!(identity.scope, "machine:M");
    }

    #[test]
    fn a_vpn_connection_keeps_the_account_scope_the_type_declares() {
        let payload = json!({"Name": "Example VPN", "TunnelType": "Ikev2", "ConnectionStatus": "Disconnected", "SplitTunneling": true, "AllUserConnection": true, "AuthenticationMethod": "Eap"});
        let result = vpn_connection(&input("pcscope.network.access", "Example VPN", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.vpn_configuration");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert_eq!(identity_field(&result, "native_id").value, "Example VPN");
        // The type's scope is the current user's, so the account the collection ran as is kept.
        assert_eq!(identity.scope, "machine:M/user:S-1-5-18");
        assert_eq!(field_of(&result, "tunnel_type").value, Some(json!("Ikev2")));
        assert_eq!(field_of(&result, "status").field_kind, FieldKind::State);
        // Get-VpnConnection has a ServerAddress; the emitter does not write it, so none is reported.
        assert_eq!(field_of(&result, "server").status, ValueStatus::NotProvided);
    }

    #[test]
    fn a_firewall_profile_keeps_what_the_class_read_and_claims_none_of_the_rest() {
        let payload = json!({
            "Name": "Domain",
            "Enabled": true,
            "DefaultInboundAction": "Block",
            "DefaultOutboundAction": "Allow",
            "AllowInboundRules": "NotConfigured",
            "AllowLocalFirewallRules": "NotConfigured",
            "AllowLocalIPsecRules": "NotConfigured",
            "LogAllowed": false,
            "LogBlocked": false,
            "LogFileName": "%systemroot%\\system32\\LogFiles\\Firewall\\domainfw.log",
            "LogMaxSizeKilobytes": 4096
        });
        let result = firewall_profiles(&input("pcscope.network.firewall-profiles", "Domain", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.firewall_profile");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert_eq!(identity_field(&result, "native_id").value, "Domain");
        assert_eq!(field_of(&result, "inbound").value, Some(json!("Block")));
        assert_eq!(field_of(&result, "outbound").value, Some(json!("Allow")));
        assert_eq!(field_of(&result, "enabled").value, Some(json!(true)));
        assert_eq!(field_of(&result, "log_path").value, Some(json!("%systemroot%\\system32\\LogFiles\\Firewall\\domainfw.log")));
        // The declared fields are exactly the five; nothing else the emitter collects is folded in.
        assert_eq!(result.observations[0].fields.len(), 5, "{:?}", result.observations[0].fields.iter().map(|f| &f.field_id).collect::<Vec<_>>());
    }

    #[test]
    fn a_firewall_rule_is_keyed_by_the_store_it_was_read_from_and_its_name() {
        let payload = json!({
            "Name": "示例规则",
            "DisplayName": "Example rule",
            "Enabled": true,
            "Direction": "Inbound",
            "Action": "Allow",
            "Profile": "Domain,Private",
            "EdgeTraversalPolicy": "Block",
            "PolicyStoreSource": "PersistentStore",
            "PolicyStoreSourceType": "Local",
            "PrimaryStatus": "OK",
            "Status": "The rule was parsed successfully from the store.",
            "filters": {"Port": {"Protocol": "TCP", "LocalPort": "Any"}, "Address": {"LocalAddress": "Any"}}
        });
        let result = firewall_rules(&input("pcscope.network.firewall-rules", "示例规则", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.firewall_rule");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        // The store is the collector's own -PolicyStore argument, and it is a declared identity key.
        assert_eq!(identity_field(&result, "policy_store").value, "ActiveStore");
        assert_eq!(identity_field(&result, "name").value, "示例规则");
        assert_eq!(field_of(&result, "direction").value, Some(json!("Inbound")));
        assert_eq!(field_of(&result, "profile").value, Some(json!("Domain,Private")));
        // The rule's own InstanceID is the join key, not an emitted field.
        assert_eq!(field_of(&result, "instance_id").status, ValueStatus::NotProvided);
        assert_eq!(identity.scope, "machine:M");
    }

    #[test]
    fn an_ipsec_rule_names_the_policy_store_in_its_scope() {
        let payload = json!({
            "Name": "Example IPsec rule",
            "DisplayName": "Example IPsec rule",
            "Enabled": true,
            "Profile": "Any",
            "Mode": "Transport",
            "InboundSecurity": "Require",
            "OutboundSecurity": "Require",
            "PolicyStoreSource": "PersistentStore",
            "PolicyStoreSourceType": "Local"
        });
        let result = ipsec_rules(&input("pcscope.network.ipsec", "Example IPsec rule", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.ipsec_rule");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert_eq!(identity_field(&result, "native_id").value, "Example IPsec rule");
        // The type's scope is the policy store's and its keys have no component for the store, so the
        // store the emitter read from is what the scope names.
        assert_eq!(identity.scope, "machine:M/policy-store:ActiveStore");
        assert_eq!(field_of(&result, "security_mode").value, Some(json!("Transport")));
        assert_eq!(field_of(&result, "policy_source").value, Some(json!("PersistentStore")));
    }

    #[test]
    fn a_share_carries_the_share_level_acl_the_collector_read() {
        let payload = json!({
            "name": "Example",
            "path": "C:\\Example",
            "description": "example share",
            "encrypt_data": false,
            "special": false,
            "access": [{"AccountName": "Everyone", "AccessControlType": "Allow", "AccessRight": "Read"}]
        });
        let result = shares(&input("pcscope.network.shares", "Example", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.smb_share");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert_eq!(identity_field(&result, "name").value, "Example");
        assert_eq!(field_of(&result, "share_acl").value.as_ref().unwrap()[0]["AccessRight"], json!("Read"));
        assert_eq!(field_of(&result, "special").value, Some(json!(false)));
        assert_eq!(identity.scope, "machine:M");
        assert!(result.actions.is_empty());
    }

    #[test]
    fn a_share_session_cannot_supply_the_server_component() {
        let payload = json!({
            "SessionId": 42,
            "ClientComputerName": "192.0.2.20",
            "ClientUserName": "EXAMPLE\\alice",
            "NumOpens": 3,
            "Dialect": "3.1.1",
            "Encrypted": true,
            "ContinuouslyAvailable": false
        });
        let result = share_sessions(&input("pcscope.network.share-sessions", "42", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.smb_session");
        // Get-SmbSession names the client and the user of each session and never the server.
        assert_eq!(identity.missing, vec!["server".to_owned()]);
        assert_eq!(identity_field(&result, "client").value, "192.0.2.20");
        assert_eq!(identity_field(&result, "user").value, "EXAMPLE\\alice");
        assert_eq!(field_of(&result, "server").status, ValueStatus::NotProvided);
        assert_eq!(field_of(&result, "open_count").value, Some(json!(3)));
        assert_eq!(field_of(&result, "protocol").value, Some(json!("3.1.1")));
        assert_eq!(field_of(&result, "encrypted").field_kind, FieldKind::State);
        // The session id is what the collector keyed the item by; the type's keys have no component
        // for it, and the item id is still the source record's own identity.
        assert!(!identity.fields.iter().any(|field| field.key == "session_id"));
    }

    #[test]
    fn a_mapped_drive_keeps_the_account_it_was_made_under() {
        let payload = json!({"LocalPath": "Z:", "RemotePath": "\\\\server\\share", "Status": "OK"});
        let result = mapped_drives(&input("pcscope.network.mapped-drives", "Z:/\\\\server\\share", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.smb_mapping");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert_eq!(identity_field(&result, "local_name").value, "Z:");
        assert_eq!(identity_field(&result, "remote_path").value, "\\\\server\\share");
        assert_eq!(field_of(&result, "status").value, Some(json!("OK")));
        // The type's scope is the user's: two accounts mapping the same letter are two mappings.
        assert_eq!(identity.scope, "machine:M/user:S-1-5-18");
        let other_account = mapped_drives(&input("pcscope.network.mapped-drives", "Z:/\\\\server\\share", json!({"LocalPath": "Z:", "RemotePath": "\\\\server\\share", "Status": "OK"})), &context_for("machine:M/user:S-1-5-21-9/schema:1/context:/shard:0"));
        assert_ne!(key_of(&identity), key_of(&other_account.observations[0].identity));
    }

    #[test]
    fn the_winrm_key_is_keyed_by_its_path_and_named_by_its_own_segment() {
        let payload = registry_item(
            "LocalMachine",
            "Registry64",
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\WSMAN",
            json!({"AllowUnencrypted": value("DWord", 0), "Port": value("DWord", 5985), "Enabled": value("DWord", 1)}),
        );
        let result = winrm_configuration(&input("pcscope.network.winrm-configuration", "LocalMachine/Registry64/SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\WSMAN", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.remote_access_configuration");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert_eq!(identity_field(&result, "path").value, "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\WSMAN");
        assert_eq!(field_of(&result, "service").value, Some(json!("WSMAN")));
        assert_eq!(field_of(&result, "values").value.as_ref().unwrap()["Port"]["value"], json!(5985));
        // A subkey the sweep reaches is its own record, keyed by its own path.
        let subkey = winrm_configuration(
            &input("pcscope.network.winrm-configuration", "LocalMachine/Registry64/SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\WSMAN\\Service", registry_item("LocalMachine", "Registry64", "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\WSMAN\\Service", json!({"RootSDDL": value("String", "O:NSG:BAD:")}))),
            &context(),
        );
        assert_ne!(key_of(&identity), key_of(&subkey.observations[0].identity));

        // The source emits registry keys only; another shape says so.
        let foreign = winrm_configuration(&input("pcscope.network.winrm-configuration", "unexpected", json!({"path": "x"})), &context());
        assert!(matches!(foreign.outcome, AdapterOutcome::NotAnObject { .. }), "{:?}", foreign.outcome);
    }

    #[test]
    fn mihomo_states_a_configuration_whose_value_name_the_material_does_not_carry() {
        let payload = json!({
            "version": "1.18.0",
            "meta": true,
            "mode": "rule",
            "log_level": "info",
            "ipv6": false,
            "allow_lan": false,
            "port": 7890,
            "socks_port": 7891,
            "mixed_port": 7892,
            "tun_enabled": false,
            "basis": "explicit local Mihomo API; no subscription, node credentials or traffic content"
        });
        let result = mihomo(&input("pcscope.apps.mihomo", "runtime", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "network.proxy_configuration");
        // The material is one classification item, not a value under a location: the location is the
        // item's own id and the value name is missing rather than filled with a setting it holds.
        assert_eq!(identity.missing, vec!["value_name".to_owned()]);
        assert_eq!(identity_field(&result, "path").value, "runtime");
        assert_eq!(field_of(&result, "path").value, Some(json!("runtime")));
        // The type declares a proxy enable flag, a proxy server and a bypass list; the running
        // instance's mode and ports are none of those, and none of the three is invented.
        for field_id in ["enabled", "server", "bypass"] {
            assert_eq!(field_of(&result, field_id).status, ValueStatus::NotProvided);
        }
    }

    #[test]
    fn a_machine_level_object_is_not_split_by_the_collecting_account_and_a_per_user_one_is() {
        let payload = registry_item(
            "LocalMachine",
            "Registry64",
            "SYSTEM\\CurrentControlSet\\Control\\Terminal Server",
            json!({"fDenyTSConnections": value("DWord", 0)}),
        );
        let by_system = remote_access_key(&input("pcscope.network.access", "key", payload.clone()), &context_for("machine:M/user:S-1-5-18/schema:1/context:/shard:0"));
        let by_user = remote_access_key(&input("pcscope.network.access", "key", payload), &context_for("machine:M/user:S-1-5-21-9/schema:1/context:/shard:0"));
        assert_eq!(key_of(&by_system.observations[0].identity), key_of(&by_user.observations[0].identity));

        let mapping = json!({"LocalPath": "Z:", "RemotePath": "\\\\server\\share"});
        let first = mapped_drives(&input("pcscope.network.mapped-drives", "Z", mapping.clone()), &context_for("machine:M/user:S-1-5-18/schema:1/context:/shard:0"));
        let second = mapped_drives(&input("pcscope.network.mapped-drives", "Z", mapping), &context_for("machine:M/user:S-1-5-21-9/schema:1/context:/shard:0"));
        assert_ne!(key_of(&first.observations[0].identity), key_of(&second.observations[0].identity));
    }

    #[test]
    fn every_source_produces_candidates_the_catalog_accepts() {
        // The catalog is the gate. A field the type does not declare, an identity value that is empty,
        // or a declared key that is neither supplied nor listed as missing is rejected here rather
        // than at projection, so every source is run through it once.
        let tcp = json!({"LocalAddress": "127.0.0.1", "LocalPort": 12345, "RemoteAddress": "192.0.2.9", "RemotePort": 443, "State": "Established", "OwningProcess": 4100, "CreationTime": "2026-09-12T00:00:00Z"});
        let cases: Vec<(&str, &str, Value)> = vec![
            ("pcscope.network.adapters", "{G}", json!({"name": "Ethernet", "status": "Up", "mac_address": "00-11-22-33-44-55"})),
            ("pcscope.network.addresses", "7/IPv4/192.0.2.10", json!({"interface_alias": "Ethernet", "address": "192.0.2.10", "prefix_length": 24, "address_family": "IPv4", "address_state": "Preferred"})),
            // The access source's three shapes, each through the catalog.
            ("pcscope.network.access", "key", registry_item("CurrentUser", "Registry64", "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings", json!({"ProxyEnable": value("DWord", 1), "ProxyServer": value("String", "192.0.2.9:8080")}))),
            ("pcscope.network.access", "key", registry_item("LocalMachine", "Registry64", "SYSTEM\\CurrentControlSet\\Control\\Terminal Server", json!({"fDenyTSConnections": value("DWord", 1)}))),
            ("pcscope.network.access", "Example VPN", json!({"Name": "Example VPN", "TunnelType": "Ikev2", "ConnectionStatus": "Disconnected"})),
            ("pcscope.network.dns-cache", "example.test/example.test/A/192.0.2.10", json!({"name": "example.test", "type": "A", "data": "192.0.2.10"})),
            ("pcscope.network.dns-configuration", "7/IPv4", json!({"InterfaceAlias": "Ethernet", "AddressFamily": "IPv4", "ServerAddresses": ["192.0.2.53"]})),
            ("pcscope.network.dns-configuration", "key", registry_item("LocalMachine", "Registry64", "SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters", json!({"SearchList": value("String", "example.test")}))),
            ("pcscope.network.firewall-profiles", "Domain", json!({"Name": "Domain", "Enabled": true, "DefaultInboundAction": "Block", "DefaultOutboundAction": "Allow", "LogFileName": "domainfw.log"})),
            ("pcscope.network.firewall-rules", "rule", json!({"Name": "rule", "Direction": "Inbound", "Action": "Allow", "Enabled": true, "Profile": "Domain"})),
            ("pcscope.network.hosts", "12/example.test", json!({"address": "192.0.2.10", "name": "example.test", "line": 12})),
            ("pcscope.network.ipsec", "rule", json!({"Name": "rule", "Mode": "Transport", "PolicyStoreSource": "PersistentStore"})),
            ("pcscope.network.mapped-drives", "Z:", json!({"LocalPath": "Z:", "RemotePath": "\\\\server\\share", "Status": "OK"})),
            ("pcscope.network.profiles", "7/example-net", json!({"Name": "example-net", "NetworkCategory": "Private"})),
            ("pcscope.network.routes", "7/0.0.0.0/0/192.0.2.1/ActiveStore", json!({"InterfaceAlias": "Ethernet", "DestinationPrefix": "0.0.0.0/0", "NextHop": "192.0.2.1", "RouteMetric": 25, "PolicyStore": "ActiveStore"})),
            ("pcscope.network.share-sessions", "42", json!({"ClientComputerName": "192.0.2.20", "ClientUserName": "EXAMPLE\\alice", "NumOpens": 3, "Dialect": "3.1.1", "Encrypted": true})),
            ("pcscope.network.shares", "Example", json!({"name": "Example", "path": "C:\\Example", "special": false, "access": [{"AccountName": "Everyone", "AccessRight": "Read"}]})),
            ("pcscope.network.tcp", "item", tcp),
            ("pcscope.network.udp", "item", json!({"LocalAddress": "0.0.0.0", "LocalPort": 5353, "OwningProcess": 1234, "CreationTime": "2026-09-12T00:00:00Z"})),
            ("pcscope.network.winrm-configuration", "key", registry_item("LocalMachine", "Registry64", "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\WSMAN", json!({"Port": value("DWord", 5985)}))),
            ("pcscope.apps.mihomo", "runtime", json!({"mode": "rule", "port": 7890, "tun_enabled": false})),
        ];
        let context = context();
        for (source, item_id, payload) in cases {
            let spec = spec_for(source).unwrap_or_else(|| panic!("{source} has a declared spec"));
            let result = (spec.build)(&input(source, item_id, payload), &context);
            assert!(matches!(result.outcome, AdapterOutcome::Adapted), "{source}: {:?}", result.outcome);
            let violations = validate(&result, &context);
            assert!(violations.is_empty(), "{source}: {violations:?}");
        }
    }

    #[test]
    fn every_source_declares_the_type_it_produces_or_decides_per_item() {
        assert_eq!(SOURCES.len(), 18, "the category's sources are the eighteen the plan assigns to it");
        for spec in SOURCES {
            if spec.source == "pcscope.network.access" {
                // Three item shapes, three declared types: the shape decides the destination.
                assert!(spec.type_id.is_none());
            } else {
                assert!(spec.type_id.is_some(), "{} names the type it produces", spec.source);
            }
        }
    }

    #[test]
    fn the_same_material_produces_the_same_candidates_twice() {
        // The expanding shape is the one worth replaying: an unstable iteration over the value table
        // would reorder or reselect the entries between two runs of the same material.
        let payload = registry_item(
            "CurrentUser",
            "Registry64",
            "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
            json!({"ProxyEnable": value("DWord", 1), "ProxyServer": value("String", "192.0.2.9:8080"), "AutoDetect": value("DWord", 0)}),
        );
        let once: Vec<String> = access(&input("pcscope.network.access", "key", payload.clone()), &context())
            .observations
            .iter()
            .map(|candidate| key_of(&candidate.identity))
            .collect();
        let twice: Vec<String> = access(&input("pcscope.network.access", "key", payload), &context())
            .observations
            .iter()
            .map(|candidate| key_of(&candidate.identity))
            .collect();
        assert_eq!(once, twice, "replaying the same material must decide the same thing");
        assert_eq!(once.len(), 3);

        // And a source that forms one identity from several payload keys replays the same way.
        let connection = json!({"LocalAddress": "127.0.0.1", "LocalPort": 12345, "RemoteAddress": "192.0.2.9", "RemotePort": 443, "OwningProcess": 4100, "CreationTime": "2026-09-12T00:00:00Z"});
        let once = key_of(&tcp_endpoints(&input("pcscope.network.tcp", "item", connection.clone()), &context()).observations[0].identity);
        let twice = key_of(&tcp_endpoints(&input("pcscope.network.tcp", "item", connection), &context()).observations[0].identity);
        assert_eq!(once, twice);
    }
}
