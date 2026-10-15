//! S20 desktop and application-integration adapters.
//!
//! Checked against the collector's own emit declarations: `domains/desktop.ps1` — the four printer
//! sources, the keyboard/input keys, the clipboard keys, the shell settings keys and the shortcut
//! walk — and `domains/applications.ps1` (`apps.ide-extensions`, `apps.onedrive-registration`). The
//! payload keys below are those emitters' own key names.
//!
//! That matters for two of these sources because the checked-in fixture disagrees with the emitter.
//! `fixtures/01-baseline.jsonl` carries a `pcscope.desktop.printing` item keyed `name`/`driver`/
//! `port`/`shared`/`status` and a `pcscope.desktop.shortcuts` item keyed `arguments_protected` —
//! the *catalog's* field ids, not the emitter's (`Name`/`DriverName`/`PortName`/`Shared`/
//! `PrinterStatus`, `arguments`). `adapter-source-survey.md` §5 records that the samples were built
//! from the type definitions rather than copied from the collector and leaves the key-name
//! reconciliation to S21; like `network.rs`, this module reads what the emitter writes.
//!
//! Ten sources map to nine declared types: the four printer sources, the two registry sources that
//! share `desktop.shell_configuration`, the input keys, the shortcut walk, the VS Code extension
//! inventory and the OneDrive registration keys.
//!
//! **Three of the things these types declare cannot be read out of this material.** They are stated
//! here rather than papered over, and each is a candidate for S21's reconciliation:
//!
//!   * `pcscope.desktop.shortcuts` feeds `desktop.shortcut`, whose strategy is `file_identity` with
//!     keys `[volume_identity, file_id]`. The collector emits the path and the link metadata and
//!     **never a volume identity or a file id** — `Collect-…`/the walk has no such value to read, and
//!     R02 forbids an adapter calling Windows again to obtain one. Both keys are therefore listed as
//!     missing on every shortcut: the path is *not* substituted for them, because the type's own
//!     `insufficient` clause says a path is not an identity ("只有路径时标记身份能力受限：路径可变").
//!     Every shortcut identity from this source is provisional until S14/S21 decides whether the
//!     collector sends the two values or the type changes strategy.
//!   * `pcscope.apps.ide-extensions` feeds `desktop.editor_extension`, whose keys are
//!     `[editor, extension_id, version]`. The emitter reads the manifest and sends `name`,
//!     `publisher`, `version` and `engines` — `ui/plugin-sources.js` describes the source as reading
//!     exactly those four — so the *editor* and the *extension id* are keys the material never
//!     states. Neither is reconstructed: the manifest's `publisher` and `name` are two fields the
//!     source did state, not an identifier it stated, and the extensions directory in the item id
//!     names the editor's config folder rather than the editor. Both are listed as missing.
//!   * `pcscope.desktop.printer-ports` declares `port_type`, and the emitter sends `Description` and
//!     `PortMonitor`. Neither is a port type as a fact of its own; naming one of the two the type's
//!     port type is the choice `virtualization.rs` refuses for a virtual machine's memory, so the
//!     field is absent and both collected facts stay in the material.
//!
//! **On fields the type declares and the material splits.** `desktop.print_job` declares one `pages`
//! field and the collector sends `total_pages` *and* `pages_printed` — two page facts, and one field
//! cannot hold both without discarding one, so it is left absent (the same rule as `port_type`
//! above).
//!
//! **On the print job's own statement.** The collector writes `document_content_collected=$false`
//! and `content_exclusion_reason='print_metadata_only_no_document_content'` for every job. The type
//! declares no field for a document name, so `document_name` stays in the payload untouched and no
//! field is invented to carry it; the adaptation repeats the source's own exclusion as a note rather
//! than contradicting it, and a test asserts the name never reaches the candidates.
//!
//! **On machine-level and per-user objects.** The printer objects are the machine's and are keyed in
//! the installation scope, so the account that ran the collection cannot split one printer into
//! several instances. The shortcut walk and the editor extension inventory run over the collecting
//! user's profile, and the sync account keys are the user's hive: those keep the scope they were
//! handed. The two registry sources that share `desktop.shell_configuration` are read from more than
//! one hive, so the hive the item itself states decides — `CurrentUser` keeps the handed scope and
//! anything else is narrowed to the installation, which is `policy.rs`'s `hive_scope` rule.
//!
//! Unlike the `value_key` types, neither of those two types declares a key for the registry view,
//! and `Registry` reads every configured path under `Registry64` *and* `Registry32`. The view is not
//! folded into the scope here: `network.rs` states the same situation for its remote-access type and
//! declines to add a component the catalog does not declare, and
//! `policy.rs`'s `value_scope` folds it only where the type's own scope text asks for it
//! (`policy.time_sync_configuration`: "安装作用域 + 注册表视图"). `desktop.shell_configuration`
//! ("用户作用域") and `desktop.input_configuration` ("用户或机器作用域") ask for no view, so a key
//! read under both views resolves to one identity. That is an open item for S21, not a repair this
//! adapter may make.
use super::*;

pub static SOURCES: &[SourceSpec] = &[
    SourceSpec { source: "pcscope.desktop.printing", type_id: Some("desktop.printer"), build: printers },
    SourceSpec { source: "pcscope.desktop.printer-drivers", type_id: Some("desktop.printer_driver"), build: printer_drivers },
    SourceSpec { source: "pcscope.desktop.printer-ports", type_id: Some("desktop.printer_port"), build: printer_ports },
    SourceSpec { source: "pcscope.desktop.print-jobs", type_id: Some("desktop.print_job"), build: print_jobs },
    SourceSpec { source: "pcscope.desktop.input", type_id: Some("desktop.input_configuration"), build: input_configuration },
    // Both registry sources describe the same thing — a configuration key — so both build the same
    // object; only the keys they read differ, and that is the collector's business, not the type's.
    SourceSpec { source: "pcscope.desktop.clipboard", type_id: Some("desktop.shell_configuration"), build: shell_configuration },
    SourceSpec { source: "pcscope.desktop.shell-settings", type_id: Some("desktop.shell_configuration"), build: shell_configuration },
    SourceSpec { source: "pcscope.desktop.shortcuts", type_id: Some("desktop.shortcut"), build: shortcuts },
    SourceSpec { source: "pcscope.apps.ide-extensions", type_id: Some("desktop.editor_extension"), build: ide_extensions },
    SourceSpec { source: "pcscope.apps.onedrive-registration", type_id: Some("desktop.sync_account"), build: sync_accounts },
];

// ---------- shared shapes ----------

/// Whether the item is one of the registry-shaped payloads
/// (`{path;hive;view;values;basis;collection}`), recognised by its value table.
fn registry_shaped(payload: &Value) -> bool {
    payload.get("values").and_then(Value::as_object).is_some()
}

/// The scope a registry item's object belongs to.
///
/// The current user's hive is the user's: which account ran the collection *is* whose settings these
/// are, so the scope handed in is kept, collecting-account segment and all. A machine hive belongs
/// to the installation — the collecting account is how the key was read, not what it configures — so
/// the machine-level narrowing `installation_scope` performs is applied. This is `policy.rs`'s rule
/// and `desktop.shell_configuration` needs it because one of its two sources reads both hives.
fn registry_scope(context: &AdapterContext, payload: &Value) -> String {
    match text(payload, "hive") {
        Some(hive) if hive.eq_ignore_ascii_case("CurrentUser") => context.collected_scope.trim().to_owned(),
        _ => installation_scope(&context.collected_scope),
    }
}

/// A registry value's data as text, for the cases where a fact the identity needs is a value of the
/// key rather than a key of the payload.
fn registry_text(payload: &Value, name: &str) -> Option<String> {
    registry_value(payload, name).and_then(|value| match value {
        Value::String(text) => Some(text),
        other => Some(other.to_string()),
    })
}

/// The last non-empty segment of a registry path: the key's own name.
fn key_name_of(path: &str) -> Option<String> {
    path.trim_end_matches('\\').rsplit('\\').next().filter(|name| !name.is_empty()).map(str::to_owned)
}

/// A field whose value the adapter already read out of the source's own text, or an explicit absence
/// when the source stated none. Never a default.
fn known_opt(field_id: &str, kind: FieldKind, value: Option<String>) -> FieldCandidate {
    match value {
        Some(value) => FieldCandidate {
            field_id: field_id.into(),
            field_kind: kind,
            value_type: "string".into(),
            value: Some(Value::String(value)),
            status: ValueStatus::Known,
            source_time: None,
            derivation: Derivation::SourceProvided,
            conflicts: Vec::new(),
        },
        None => FieldCandidate {
            field_id: field_id.into(),
            field_kind: kind,
            value: None,
            value_type: "unknown".into(),
            status: ValueStatus::NotProvided,
            source_time: None,
            derivation: Derivation::SourceProvided,
            conflicts: Vec::new(),
        },
    }
}

// ---------- pcscope.desktop.printing ----------

/// `Get-Printer`, one item per printer keyed by the printer's name.
///
/// `desktop.printer` is a `named_entity`: the name is the identity and there is no identifier to fall
/// back on, so a printer the item did not name has a provisional identity rather than a name taken
/// from anywhere else. The scope is the installation: printers belong to the machine, and which
/// account ran the collection is how the spooler was reached.
///
/// `Type`, `ShareName`, `Published` and `RenderingMode` are collected and the type declares no field
/// for them, so they are not mapped and stay in the material.
fn printers(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    let scope = installation_scope(&context.collected_scope);
    let identity = identity("desktop.printer", &scope, &input.evidence.id, &[("name", text(payload, "Name"))]);
    let fields = vec![
        field(payload, "name", "Name", FieldKind::Identity),
        field(payload, "driver", "DriverName", FieldKind::Configuration),
        field(payload, "port", "PortName", FieldKind::Configuration),
        field(payload, "shared", "Shared", FieldKind::Configuration),
        field(payload, "status", "PrinterStatus", FieldKind::State),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.desktop.printer-drivers ----------

/// `Get-PrinterDriver`, one item per driver per environment.
///
/// `desktop.printer_driver` is a `composite_native` whose two components are the driver's name and
/// the environment it is installed for — the collector keys its item by exactly that pair
/// (`"$($_.Name)/$($_.PrinterEnvironment)"`), because the same driver name can be installed once for
/// each environment. A component the material omits is listed as missing, which is the provisional
/// identity the type's `insufficient` clause describes; the item id is not substituted for it.
///
/// `MajorVersion`, `ConfigFile` and `DataFile` are collected and the type declares no field for them.
fn printer_drivers(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    let scope = installation_scope(&context.collected_scope);
    let identity = identity(
        "desktop.printer_driver",
        &scope,
        &input.evidence.id,
        &[("name", text(payload, "Name")), ("environment", text(payload, "PrinterEnvironment"))],
    );
    let fields = vec![
        field(payload, "name", "Name", FieldKind::Identity),
        field(payload, "environment", "PrinterEnvironment", FieldKind::Identity),
        field(payload, "manufacturer", "Manufacturer", FieldKind::Configuration),
        field(payload, "driver_path", "DriverPath", FieldKind::Configuration),
        field(payload, "inf", "InfPath", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.desktop.printer-ports ----------

/// `Get-PrinterPort`, one item per port keyed by the port's name.
///
/// `port_type` is left absent. The type declares a port *type*; the collector sends a `Description`
/// and a `PortMonitor`, each a distinct thing the Windows print architecture names separately, and
/// neither of them is stated as the port's type. Naming one of the two the type's port type would
/// discard the other and assert a classification the source did not make, which is the choice
/// `virtualization.rs` refuses for a virtual machine's memory. Both facts stay in the material, and
/// S21 can decide whether the model wants the monitor or the description under that field.
///
/// `SNMPEnabled` is collected and the type declares no field for it.
fn printer_ports(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    let scope = installation_scope(&context.collected_scope);
    let identity = identity("desktop.printer_port", &scope, &input.evidence.id, &[("name", text(payload, "Name"))]);
    let fields = vec![
        field(payload, "name", "Name", FieldKind::Identity),
        // No payload key states the port's type; see the note above.
        field(payload, "port_type", "port_type", FieldKind::Configuration),
        field(payload, "host", "PrinterHostAddress", FieldKind::Configuration),
        field(payload, "protocol", "Protocol", FieldKind::Configuration),
        field(payload, "port_number", "PortNumber", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.desktop.print-jobs ----------

/// `Get-PrintJob` for every printer, one item per job.
///
/// `desktop.print_job` is a `composite_native` of the printer and the job id, and the collector keys
/// its item by the printer, the job id and the submission time — a job id is only unique within its
/// printer's queue, which is why the printer is half of the identity. The submission time is a field
/// the type declares as a configuration value, not an identity component, so it is mapped there.
///
/// `pages` is left absent: the collector sends `total_pages` and `pages_printed`, two different page
/// facts, and one declared field cannot hold both without discarding one.
///
/// **The source's own statement about document content travels with the material.** Every job
/// carries `document_content_collected=$false` and
/// `content_exclusion_reason='print_metadata_only_no_document_content'`: only print metadata is
/// collected and never the document. The type declares no field for a document name, so
/// `document_name` is not mapped and no field is invented for it — it stays in the payload. The
/// exclusion is repeated as a note on the result, which is the one place this adapter may restate it
/// without turning it into a value.
fn print_jobs(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    let scope = installation_scope(&context.collected_scope);
    let identity = identity(
        "desktop.print_job",
        &scope,
        &input.evidence.id,
        &[("printer", text(payload, "printer")), ("job_id", text(payload, "id"))],
    );
    let fields = vec![
        field(payload, "printer", "printer", FieldKind::Identity),
        field(payload, "job_id", "id", FieldKind::Identity),
        field(payload, "submitted_at", "submitted_at", FieldKind::Configuration),
        field(payload, "size", "size", FieldKind::Configuration),
        // `total_pages` and `pages_printed` are both in the material; see the note above.
        field(payload, "pages", "pages", FieldKind::Configuration),
        field(payload, "status", "status", FieldKind::State),
    ];
    let mut result = object_candidate(input, context, identity, fields);
    if let Some(statement) = content_exclusion(payload) {
        result.notes.push(statement);
    }
    result
}

/// The source's own statement that it collects print metadata only, echoed when the material makes
/// it — never asserted on the source's behalf.
fn content_exclusion(payload: &Value) -> Option<String> {
    let excluded = matches!(payload.get("document_content_collected"), Some(Value::Bool(false)));
    let reason = text(payload, "content_exclusion_reason");
    if !excluded && reason.is_none() {
        return None;
    }
    Some(match reason {
        Some(reason) => format!(
            "the source states that it collects print metadata only ({reason}); the document name is kept in the material and no field carries it"
        ),
        None => "the source states that it collects print metadata only; the document name is kept in the material and no field carries it".to_owned(),
    })
}

// ---------- pcscope.desktop.input ----------

/// The three keyboard and input keys: the user's `Keyboard Layout` key, the machine's
/// `Keyboard Layouts` tree and the keyboard class key's filter drivers.
///
/// `desktop.input_configuration` is a `native_path` type, so the key's own path is the identity and
/// one key is one object — including the subkeys the sweep reaches, each of which is a layout or a
/// filter configuration in its own right. The declared fields are the values those keys actually
/// hold, read through `registry_field` so a value the collector masked stays masked. The user's
/// `Keyboard Layout` key holds preload entries under names (`''`, `'1'`…) that no declared field
/// names: they stay in the material rather than being mapped onto `layout_file`, which is the value
/// the `Keyboard Layouts` tree states under that name.
///
/// The source reads both hives under both registry views. The hive decides the scope (see
/// `registry_scope`); the view is the open item the module doc describes.
fn input_configuration(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    if !registry_shaped(payload) {
        return AdapterResult::not_an_object("input configuration item is not a registry key; this source emits none");
    }
    let scope = registry_scope(context, payload);
    let identity = identity("desktop.input_configuration", &scope, &input.evidence.id, &[("path", text(payload, "path"))]);
    let fields = vec![
        field(payload, "path", "path", FieldKind::Identity),
        registry_field(payload, "layout_file", "Layout File", FieldKind::Configuration),
        registry_field(payload, "ime_file", "Ime File", FieldKind::Configuration),
        registry_field(payload, "upper_filters", "UpperFilters", FieldKind::Configuration),
        registry_field(payload, "lower_filters", "LowerFilters", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.desktop.clipboard and pcscope.desktop.shell-settings ----------

/// The clipboard keys and the desktop/Explorer settings keys: two sources, one type, one shape.
///
/// `desktop.shell_configuration` is a `native_path` type whose keys are the scope and the
/// configuration path, so a registry key is one object and its whole value table is the `value`
/// field — the type declares one `value` for a configuration path rather than one field per setting,
/// exactly as `policy.rs` maps the time service key's table. Expanding the key into one candidate per
/// value would give every value of one key the same identity, which is the identity's answer, not a
/// set of objects.
///
/// `name` is the key's own name, the last segment of the path the collector read: that is the
/// source's own statement of which configuration the object is, the way `network.rs` reads the
/// service out of a remote-access key. It is not the name of one of the values, which would be
/// picking one setting out of the table and calling it the whole.
///
/// The three shell-settings keys are all the current user's, so this source's objects keep the scope
/// they were handed — the collecting user's, with the user segment intact. The clipboard source also
/// reads a machine policy key, which `registry_scope` narrows to the installation. Neither source's
/// type declares an action, and neither here states one: `execution.clipboard_changed` is a
/// clipboard *content* change, and `source-map.cjs` assigns both sources the `observation`
/// destination and the material states that the clipboard body is never read.
fn shell_configuration(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    if !registry_shaped(payload) {
        return AdapterResult::not_an_object("shell configuration item is not a registry key; both sources that map to this type emit none other");
    }
    let scope = registry_scope(context, payload);
    let path = text(payload, "path");
    let identity = identity("desktop.shell_configuration", &scope, &input.evidence.id, &[("path", path.clone())]);
    let fields = vec![
        field(payload, "path", "path", FieldKind::Identity),
        known_opt("name", FieldKind::Configuration, path.as_deref().and_then(key_name_of)),
        field(payload, "value", "values", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.desktop.shortcuts ----------

/// The shortcut walk over the user's and the all-users' desktop and start menu folders.
///
/// **The type's identity strategy cannot be satisfied from this material.** `desktop.shortcut` is a
/// `file_identity`: its keys are `[volume_identity, file_id]`, and this collector sends neither —
/// it sends the path and the link metadata. Both keys are therefore listed as missing on every
/// shortcut, and the path is deliberately not substituted for them: the type's own normalization
/// says the path is for display ("路径保留原大小写，仅用于展示"), its `insufficient` clause says a
/// path is not an identity because a rename or a move changes it, and R02 forbids the adapter
/// fetching the two values from Windows itself. Every shortcut here is therefore a provisional
/// identity until S14/S21 decides whether the collector sends `volume_identity` and `file_id` or the
/// type changes strategy. This is the same gap `adapter-source-survey.md` §3.1 records, and this
/// adapter neither closes it nor hides it.
///
/// Two item shapes occur and both are shortcuts: the walk emits a `.lnk` it parsed with its target,
/// working directory, arguments, icon and hotkey, and — for a reparse point — only the path and the
/// attributes, because the walk states it does not follow a reparse target outside its scan policy.
/// The second shape is a shortcut whose link metadata the source did not state, so those fields are
/// absent rather than empty; `attributes`, `collection`, `description`, `window_style` and `basis`
/// are collected and the type declares no field for them.
///
/// The scope is the one handed in. `desktop.shortcut` declares the user's scope, and the walk's
/// items carry nothing that says which of its four folders a path came from — the all-users desktop
/// and start menu folders are walked too, and a shortcut placed there belongs to the installation
/// rather than to the collecting user. Separating them would mean this adapter deciding what
/// `C:\ProgramData\Microsoft\Windows\Start Menu` means, which is exactly the kind of inference R02
/// forbids; the material states no such thing, so the type's scope is used as declared and the
/// limitation is recorded here for S21.
///
/// The walk's own basis — "existing shortcut metadata; target never executed" — travels with the
/// material, and no action is invented from a shortcut's existence. The type declares none.
fn shortcuts(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    let scope = context.collected_scope.trim().to_owned();
    let identity = identity(
        "desktop.shortcut",
        &scope,
        &input.evidence.id,
        // Neither value exists in the material; see the note above. Neither is replaced by the path.
        &[("volume_identity", None), ("file_id", None)],
    );
    let fields = vec![
        field(payload, "path", "path", FieldKind::Identity),
        field(payload, "target", "target", FieldKind::Configuration),
        field(payload, "working_directory", "working_directory", FieldKind::Configuration),
        field(payload, "arguments", "arguments", FieldKind::Configuration),
        field(payload, "icon", "icon", FieldKind::Configuration),
        field(payload, "hotkey", "hotkey", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

// ---------- pcscope.apps.ide-extensions ----------

/// The installed VS Code extensions, one item per extension directory whose manifest was read.
///
/// `desktop.editor_extension` is a `composite_native` of the editor, the extension id and the
/// version. The emitter reads the manifest and sends `name`, `publisher`, `version` and `engines`,
/// which is what the source's own description says it reads; it sends no editor and no extension id.
/// Neither is reconstructed here. `publisher` and `name` are two fields the source did state — they
/// are not an identifier it stated — and the item id's directory names the editor's configuration
/// folder (`.vscode` or `.vscode-insiders`) rather than the editor, so reading `editor` out of a path
/// would be naming an object after a directory. Both are listed as missing, which is what makes the
/// identity provisional rather than wrong, and S21 has to decide between the collector sending the
/// two values and the type changing keys. `engines` and `basis` are mapped where a field exists (only
/// `engines` has one).
///
/// The scope is the one handed in: the type's scope is "编辑器 + 用户配置文件", and the walk is over
/// the collecting user's `%USERPROFILE%`.
fn ide_extensions(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    if !is_extension_manifest(payload) {
        return AdapterResult::not_an_object("editor extension item states none of name, publisher, version or engines; it is not a manifest reading");
    }
    let scope = context.collected_scope.trim().to_owned();
    let identity = identity(
        "desktop.editor_extension",
        &scope,
        &input.evidence.id,
        // The material states neither; see the note above.
        &[("editor", None), ("extension_id", None), ("version", text(payload, "version"))],
    );
    let fields = vec![
        field(payload, "editor", "editor", FieldKind::Identity),
        field(payload, "extension_id", "extension_id", FieldKind::Identity),
        field(payload, "version", "version", FieldKind::Identity),
        field(payload, "name", "name", FieldKind::Configuration),
        field(payload, "publisher", "publisher", FieldKind::Configuration),
        field(payload, "engines", "engines", FieldKind::Configuration),
    ];
    object_candidate(input, context, identity, fields)
}

/// Whether the payload is a manifest reading at all. The emitter writes one key per manifest field,
/// so an item carrying none of them is not the material this source produces — the check is on the
/// source's own keys, not on a guess about what an extension looks like.
fn is_extension_manifest(payload: &Value) -> bool {
    payload.get("name").is_some() || payload.get("publisher").is_some() || payload.get("version").is_some() || payload.get("engines").is_some()
}

// ---------- pcscope.apps.onedrive-registration ----------

/// The OneDrive registration keys: the user's `OneDrive\Accounts` tree, the user's
/// `SyncEngines\Providers` tree and the machine's OneDrive policy key.
///
/// `desktop.sync_account` is a `native_identifier` whose key is `native_id`, and the source's own
/// description says it reads "账户、挂载与策略字段" — the account, the mount point and the policy.
/// The identifier is therefore taken from what the material states as an identifier — a value of the
/// key, read out of the collector's value table — the sync engine's `NamespaceID` where the item
/// carries one, otherwise the account's `AccountName`. A
/// `DisplayName` is never used for it, which is what the type's `insufficient` clause forbids; an
/// item that states neither leaves `native_id` missing and its identity provisional.
///
/// The path decides which of the three registrations an item is, because the collector emits the
/// container keys as well as what they contain:
///
///   * below `\Accounts\` — one account's registration. `account` is that registration's
///     `AccountName`. `UserEmail`, `DisplayName`, `TenantId` and `ConfiguredTenantId` are collected
///     and none of them is mapped: the type declares one account field and these are the same
///     account stated several ways, so naming one would be the choice this module refuses
///     elsewhere.
///   * below `\Providers\<provider>\` — the sync engine's registration of one synced folder, which is
///     the only shape that states a `MountPoint`. `provider` is the path segment the collector wrote
///     under `Providers`, read from the source's own text the way `policy.rs` reads a browser out of
///     a policy path. This shape states its account only as a `DisplayName`, which stays in the
///     material: a display name is not the account's identifier, and the type says so.
///   * under the machine's `\Policies\Microsoft\OneDrive` key — the registration of the client's
///     policy, mapped because the source states policy as one of the three things it reads and the
///     type declares a `policy` field. Its whole value table is that field, for the same reason
///     `shell_configuration` carries a key's table: the type declares one policy for the
///     registration, not one field per switch.
///
/// The two container keys (`…\OneDrive\Accounts` itself and `…\Providers\OneDrive`) register
/// nothing of their own: they are the lists, not an entry. They are skipped with that reason, and
/// whatever values they hold stay in the material rather than becoming an object with no account.
///
/// The scope follows the hive: the user's keys keep the scope they were handed, and the machine
/// policy key is narrowed to the installation.
fn sync_accounts(input: &EvidenceInput, context: &AdapterContext) -> AdapterResult {
    let payload = &input.payload;
    let Some(path) = text(payload, "path").filter(|_| registry_shaped(payload)) else {
        return AdapterResult::not_an_object("OneDrive registration item is not a registry key; this source emits none");
    };
    if !is_registration(&path) {
        return AdapterResult {
            outcome: AdapterOutcome::Skipped {
                reason: format!("{path} is a container key; it registers no account, mount or policy of its own"),
            },
            ..AdapterResult::adapted()
        };
    }
    let scope = registry_scope(context, payload);
    let native_id = registry_text(payload, "NamespaceID").or_else(|| registry_text(payload, "AccountName"));
    let identity = identity("desktop.sync_account", &scope, &input.evidence.id, &[("native_id", native_id)]);
    let fields = vec![
        known_opt("provider", FieldKind::Identity, provider_of(&path)),
        registry_field(payload, "account", "AccountName", FieldKind::Configuration),
        registry_field(payload, "mount_point", "MountPoint", FieldKind::Configuration),
        if is_policy_path(&path) {
            field(payload, "policy", "values", FieldKind::Configuration)
        } else {
            // The other two shapes register an account or a synced folder; neither states a policy.
            field(payload, "policy", "policy", FieldKind::Configuration)
        },
    ];
    object_candidate(input, context, identity, fields)
}

/// Whether the key is one of the three registrations this source reads, rather than one of the two
/// container keys it also emits. The shape is the collector's own: it walks one level below
/// `…\OneDrive\Accounts` and one level below `…\SyncEngines\Providers\OneDrive`, and reads the
/// machine policy key itself.
fn is_registration(path: &str) -> bool {
    let segments: Vec<&str> = path.split('\\').filter(|segment| !segment.trim().is_empty()).collect();
    let below = |marker: &str, offset: usize| {
        segments
            .iter()
            .position(|segment| segment.eq_ignore_ascii_case(marker))
            .and_then(|index| segments.get(index + offset))
            .is_some()
    };
    below("Accounts", 1) || below("Providers", 2) || is_policy_path(path)
}

/// Whether the key is the machine's OneDrive policy key, which is the one shape that states a policy.
fn is_policy_path(path: &str) -> bool {
    path.split('\\').filter(|segment| !segment.trim().is_empty()).map(str::to_ascii_lowercase).collect::<Vec<String>>().windows(3).any(|window| {
        window[0] == "policies" && window[1] == "microsoft" && window[2] == "onedrive"
    })
}

/// The sync provider a key belongs to, read out of the registry path the collector wrote.
///
/// `Software\SyncEngines\Providers\<provider>\…` states it directly as the segment after
/// `Providers`; the OneDrive keys state it as their own product segment. A path that names neither
/// leaves the field absent rather than naming a provider this adapter happens to know about.
fn provider_of(path: &str) -> Option<String> {
    let segments: Vec<&str> = path.split('\\').filter(|segment| !segment.trim().is_empty()).collect();
    if let Some(index) = segments.iter().position(|segment| segment.eq_ignore_ascii_case("Providers")) {
        return segments.get(index + 1).map(|provider| (*provider).to_owned());
    }
    segments
        .iter()
        .position(|segment| segment.eq_ignore_ascii_case("OneDrive"))
        .map(|index| segments[index].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::EvidenceKind;
    use serde_json::json;

    const SCOPE: &str = "machine:M/user:S-1-5-18/schema:1/context:/shard:0";

    fn input(source: &str, item_id: &str, payload: Value) -> EvidenceInput {
        EvidenceInput {
            source: source.into(),
            item_id: item_id.into(),
            structure_version: "v1".into(),
            source_scope: SCOPE.into(),
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
        context_for(SCOPE)
    }

    fn key_of(candidate: &IdentityCandidate) -> String {
        let mut parts: Vec<String> = candidate.fields.iter().map(|field| format!("{}={}", field.key, field.value)).collect();
        parts.sort();
        parts.join("|")
    }

    fn field_of<'a>(result: &'a AdapterResult, field_id: &str) -> &'a FieldCandidate {
        result.observations[0].fields.iter().find(|field| field.field_id == field_id).expect("field is mapped")
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

    /// One item of every source this category declares, so a single check can hold every mapping to
    /// the catalog the types come from.
    fn samples() -> Vec<(&'static str, &'static str, Value)> {
        vec![
            (
                "pcscope.desktop.printing",
                "示例打印机",
                json!({"Name": "示例打印机", "Type": 4, "DriverName": "示例驱动", "PortName": "示例端口", "Shared": false, "ShareName": null, "Published": false, "PrinterStatus": "Idle", "RenderingMode": 1}),
            ),
            (
                "pcscope.desktop.printer-drivers",
                "示例驱动/Windows x64",
                json!({"Name": "示例驱动", "Manufacturer": "示例制造商", "PrinterEnvironment": "Windows x64", "MajorVersion": 3, "DriverPath": "C:\\Windows\\System32\\spool\\drivers\\x64\\3", "InfPath": "prnms001.inf", "ConfigFile": "unidrv.dll", "DataFile": "unidrv.hlp"}),
            ),
            (
                "pcscope.desktop.printer-ports",
                "示例端口",
                json!({"Name": "示例端口", "Description": "Standard TCP/IP Port", "PortMonitor": "Standard TCP/IP Port", "PrinterHostAddress": "192.0.2.10", "PortNumber": 9100, "Protocol": 1, "SNMPEnabled": true}),
            ),
            (
                "pcscope.desktop.print-jobs",
                "示例打印机/7/2026-09-12T00:00:00.0000000Z",
                json!({"printer": "示例打印机", "id": 7, "submitted_at": "2026-09-12T00:00:00.0000000Z", "status": "Printing", "size": 4096, "total_pages": 2, "pages_printed": 1, "document_name": "文档.txt", "document_content_collected": false, "content_exclusion_reason": "print_metadata_only_no_document_content"}),
            ),
            (
                "pcscope.desktop.input",
                "LocalMachine/Registry64/SYSTEM\\CurrentControlSet\\Control\\Keyboard Layouts\\00000409",
                registry_item("LocalMachine", "Registry64", "SYSTEM\\CurrentControlSet\\Control\\Keyboard Layouts\\00000409", json!({"Layout File": value("String", "kbdus.dll"), "Layout Text": value("String", "US"), "Ime File": value("String", "")})),
            ),
            (
                "pcscope.desktop.clipboard",
                "CurrentUser/Registry64/Software\\Microsoft\\Clipboard",
                registry_item("CurrentUser", "Registry64", "Software\\Microsoft\\Clipboard", json!({"EnableClipboardHistory": value("DWord", 1)})),
            ),
            (
                "pcscope.desktop.shell-settings",
                "CurrentUser/Registry64/Control Panel\\Desktop",
                registry_item("CurrentUser", "Registry64", "Control Panel\\Desktop", json!({"Wallpaper": value("String", "C:\\示例\\壁纸.jpg"), "ScreenSaveActive": value("String", "1")})),
            ),
            (
                "pcscope.desktop.shortcuts",
                "c:\\示例\\桌面\\示例快捷方式.lnk",
                json!({"path": "C:\\示例\\桌面\\示例快捷方式.lnk", "target": "C:\\示例\\程序\\示例程序.exe", "working_directory": "C:\\示例\\程序", "arguments": "--示例参数", "description": null, "window_style": 1, "icon": "C:\\示例\\程序\\示例程序.exe,0", "hotkey": "", "basis": "existing shortcut metadata; target never executed"}),
            ),
            (
                "pcscope.apps.ide-extensions",
                "C:\\Users\\u\\.vscode\\extensions\\example.tool-1.2.3",
                json!({"name": "tool", "publisher": "example", "version": "1.2.3", "engines": {"vscode": "^1.80.0"}, "basis": "installed manifest; does not establish activation"}),
            ),
            (
                "pcscope.apps.onedrive-registration",
                "CurrentUser/Registry64/Software\\Microsoft\\OneDrive\\Accounts\\Personal",
                registry_item("CurrentUser", "Registry64", "Software\\Microsoft\\OneDrive\\Accounts\\Personal", json!({"AccountName": value("String", "user@example.test"), "DisplayName": value("String", "示例账户")})),
            ),
            (
                "pcscope.apps.onedrive-registration",
                "LocalMachine/Registry64/Software\\Policies\\Microsoft\\OneDrive",
                registry_item("LocalMachine", "Registry64", "Software\\Policies\\Microsoft\\OneDrive", json!({"DisableFileSyncNGSC": value("DWord", 0)})),
            ),
        ]
    }

    #[test]
    fn every_declared_source_is_registered_under_this_category() {
        let declared: Vec<&str> = SOURCES.iter().map(|spec| spec.source).collect();
        for source in [
            "pcscope.desktop.printing",
            "pcscope.desktop.printer-drivers",
            "pcscope.desktop.printer-ports",
            "pcscope.desktop.print-jobs",
            "pcscope.desktop.input",
            "pcscope.desktop.clipboard",
            "pcscope.desktop.shell-settings",
            "pcscope.desktop.shortcuts",
            "pcscope.apps.ide-extensions",
            "pcscope.apps.onedrive-registration",
        ] {
            assert!(declared.contains(&source), "{source} is declared");
            assert!(adapter_for(source).is_some(), "{source} has an adapter");
        }
        assert_eq!(declared.len(), 10, "ten sources");
        // ...which is nine declared types, because two sources describe one thing.
        let mut types: Vec<&str> = SOURCES.iter().filter_map(|spec| spec.type_id).collect();
        types.sort_unstable();
        types.dedup();
        assert_eq!(types.len(), 9, "{types:?}");
    }

    #[test]
    fn every_field_every_source_maps_is_declared_by_the_type_it_names() {
        let context = context();
        for (source, item_id, payload) in samples() {
            let spec = spec_for(source).expect("the source is declared in the category registry");
            let type_id = spec.type_id.expect("every source in this category produces one type");
            let result = (spec.build)(&input(source, item_id, payload), &context);
            assert_eq!(result.outcome, AdapterOutcome::Adapted, "{source} did not adapt its material");
            assert_eq!(result.observations.len(), 1, "{source}");
            assert_eq!(result.observations[0].identity.type_id, type_id, "{source}");
            assert!(result.complete(), "{source} produced an incomplete result");
            let violations = validate(&result, &context);
            assert!(violations.is_empty(), "{source}: {violations:?}");
        }
    }

    // ---------- printers ----------

    #[test]
    fn a_printer_is_keyed_by_its_name_in_the_installation_scope() {
        let payload = json!({"Name": "示例打印机", "DriverName": "示例驱动", "PortName": "示例端口", "Shared": true, "PrinterStatus": "Idle", "Type": 4, "ShareName": "share", "Published": false, "RenderingMode": 1});
        let result = printers(&input("pcscope.desktop.printing", "示例打印机", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "desktop.printer");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "name" && field.value == "示例打印机"));
        assert_eq!(field_of(&result, "driver").value, Some(json!("示例驱动")));
        assert_eq!(field_of(&result, "port").value, Some(json!("示例端口")));
        assert_eq!(field_of(&result, "shared").value, Some(json!(true)));
        assert_eq!(field_of(&result, "status").value, Some(json!("Idle")));
        assert_eq!(field_of(&result, "status").field_kind, FieldKind::State);
    }

    #[test]
    fn a_machine_level_printer_does_not_depend_on_the_collecting_account() {
        let payload = json!({"Name": "示例打印机", "PrinterStatus": "Idle"});
        let by_system = printers(&input("pcscope.desktop.printing", "示例打印机", payload.clone()), &context_for("machine:M/user:S-1-5-18/schema:1/context:/shard:0"));
        let by_user = printers(&input("pcscope.desktop.printing", "示例打印机", payload), &context_for("machine:M/user:S-1-5-21-9/schema:1/context:/shard:0"));
        assert_eq!(key_of(&by_system.observations[0].identity), key_of(&by_user.observations[0].identity));
        assert!(!by_system.observations[0].identity.scope.contains("S-1-5-18"), "{}", by_system.observations[0].identity.scope);
    }

    #[test]
    fn a_printer_the_source_did_not_describe_has_absent_fields_not_defaults() {
        let result = printers(&input("pcscope.desktop.printing", "示例打印机", json!({"Name": "示例打印机"})), &context());
        // No driver, no port, no sharing state: absent, never an empty string or a false.
        for field_id in ["driver", "port", "shared", "status"] {
            assert_eq!(field_of(&result, field_id).status, ValueStatus::NotProvided, "{field_id}");
            assert!(field_of(&result, field_id).value.is_none(), "{field_id}");
        }
        // A printer whose name the item did not state has no identity name to be keyed by, and none
        // is taken from the item id.
        let unnamed = printers(&input("pcscope.desktop.printing", "示例打印机", json!({"PrinterStatus": "Idle"})), &context());
        assert_eq!(unnamed.observations[0].identity.missing, vec!["name".to_owned()]);
        assert!(field_of(&unnamed, "status").value.is_some(), "the rest of the material is still read");
    }

    // ---------- printer drivers ----------

    #[test]
    fn a_printer_driver_is_keyed_by_its_name_and_environment_together() {
        let payload = json!({"Name": "示例驱动", "PrinterEnvironment": "Windows x64", "Manufacturer": "示例制造商", "DriverPath": "C:\\spool\\x64\\3", "InfPath": "prnms001.inf", "MajorVersion": 3, "ConfigFile": "unidrv.dll"});
        let result = printer_drivers(&input("pcscope.desktop.printer-drivers", "示例驱动/Windows x64", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "desktop.printer_driver");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "name" && field.value == "示例驱动"));
        assert!(identity.fields.iter().any(|field| field.key == "environment" && field.value == "Windows x64"));
        assert_eq!(field_of(&result, "manufacturer").value, Some(json!("示例制造商")));
        assert_eq!(field_of(&result, "driver_path").value, Some(json!("C:\\spool\\x64\\3")));
        assert_eq!(field_of(&result, "inf").value, Some(json!("prnms001.inf")));
    }

    #[test]
    fn a_driver_without_an_environment_has_an_incomplete_identity_rather_than_a_guessed_one() {
        let result = printer_drivers(&input("pcscope.desktop.printer-drivers", "示例驱动/Windows x64", json!({"Name": "示例驱动"})), &context());
        // The item id holds the pair, and it is not substituted for the missing half.
        assert_eq!(result.observations[0].identity.missing, vec!["environment".to_owned()]);
    }

    // ---------- printer ports ----------

    #[test]
    fn a_printer_port_is_keyed_by_its_name_and_states_no_port_type() {
        let payload = json!({"Name": "示例端口", "Description": "Standard TCP/IP Port", "PortMonitor": "Standard TCP/IP Port", "PrinterHostAddress": "192.0.2.10", "PortNumber": 9100, "Protocol": 1, "SNMPEnabled": true});
        let material = input("pcscope.desktop.printer-ports", "示例端口", payload);
        let result = printer_ports(&material, &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "desktop.printer_port");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert_eq!(field_of(&result, "host").value, Some(json!("192.0.2.10")));
        assert_eq!(field_of(&result, "protocol").value, Some(json!(1)));
        assert_eq!(field_of(&result, "port_number").value, Some(json!(9100)));
        // Two collected facts could each be called the port's type, so neither is: the field is
        // absent and both stay in the material.
        assert_eq!(field_of(&result, "port_type").status, ValueStatus::NotProvided);
        assert!(field_of(&result, "port_type").value.is_none());
        // Both facts the field could have been read from are still in the material they came with.
        assert!(material.payload["Description"].is_string() && material.payload["PortMonitor"].is_string());
        assert_eq!(material.payload["SNMPEnabled"], json!(true), "a key no field names is not rewritten");
    }

    // ---------- print jobs ----------

    #[test]
    fn a_print_job_is_keyed_by_its_printer_and_job_id() {
        let payload = json!({"printer": "示例打印机", "id": 7, "submitted_at": "2026-09-12T00:00:00.0000000Z", "status": "Printing", "size": 4096, "total_pages": 2, "pages_printed": 1});
        let result = print_jobs(&input("pcscope.desktop.print-jobs", "示例打印机/7/2026-09-12T00:00:00.0000000Z", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "desktop.print_job");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        // A job id is unique within its printer's queue, so both halves are the identity.
        assert!(identity.fields.iter().any(|field| field.key == "printer" && field.value == "示例打印机"));
        assert!(identity.fields.iter().any(|field| field.key == "job_id" && field.value == "7"));
        assert_eq!(field_of(&result, "submitted_at").value, Some(json!("2026-09-12T00:00:00.0000000Z")));
        assert_eq!(field_of(&result, "size").value, Some(json!(4096)));
        assert_eq!(field_of(&result, "status").value, Some(json!("Printing")));
        // Two page facts, one declared field: neither is picked.
        assert_eq!(field_of(&result, "pages").status, ValueStatus::NotProvided);
        assert!(field_of(&result, "pages").value.is_none());
    }

    #[test]
    fn a_print_job_never_carries_the_document_name_its_source_excludes() {
        let payload = json!({"printer": "示例打印机", "id": 7, "submitted_at": "2026-09-12T00:00:00.0000000Z", "status": "Printing", "size": 4096, "document_name": "示例文档.txt", "document_content_collected": false, "content_exclusion_reason": "print_metadata_only_no_document_content"});
        let input = input("pcscope.desktop.print-jobs", "示例打印机/7/2026-09-12T00:00:00.0000000Z", payload);
        let result = print_jobs(&input, &context());
        // The source states that only print metadata is collected. The document name is not a field
        // of the type and never becomes one, and no candidate carries it.
        let serialised = serde_json::to_string(&result).unwrap();
        assert!(!serialised.contains("示例文档.txt"), "{serialised}");
        assert!(result.notes.iter().any(|note| note.contains("print_metadata_only_no_document_content")), "{:?}", result.notes);
        // The statement is the source's, and it is still in the material it came with.
        assert_eq!(input.payload["document_content_collected"], json!(false));
        assert!(input.payload["document_name"].is_string());
    }

    #[test]
    fn a_job_that_states_no_exclusion_is_not_told_that_it_has_one() {
        let result = print_jobs(&input("pcscope.desktop.print-jobs", "p/1/t", json!({"printer": "p", "id": 1})), &context());
        assert!(result.notes.is_empty(), "{:?}", result.notes);
    }

    // ---------- input configuration ----------

    #[test]
    fn an_input_key_is_keyed_by_its_path_and_reads_the_layout_values() {
        let payload = registry_item(
            "LocalMachine",
            "Registry64",
            "SYSTEM\\CurrentControlSet\\Control\\Keyboard Layouts\\00000409",
            json!({"Layout File": value("String", "kbdus.dll"), "Layout Text": value("String", "US"), "Ime File": value("String", "ime.dll")}),
        );
        let result = input_configuration(&input("pcscope.desktop.input", "LocalMachine/Registry64/Keyboard Layouts/00000409", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "desktop.input_configuration");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "path" && field.value.ends_with("00000409")));
        assert_eq!(field_of(&result, "layout_file").value, Some(json!("kbdus.dll")));
        assert_eq!(field_of(&result, "ime_file").value, Some(json!("ime.dll")));
        // This key states no filter drivers.
        assert_eq!(field_of(&result, "upper_filters").status, ValueStatus::NotProvided);
        // `Layout Text` is collected and the type declares no field for it.
        assert!(result.observations[0].fields.iter().all(|field| field.field_id != "layout_text"));
    }

    #[test]
    fn an_input_key_reads_the_filter_drivers_and_keeps_a_masked_value_masked() {
        let payload = registry_item(
            "LocalMachine",
            "Registry64",
            "SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e96b-e325-11ce-bfc1-08002be10318}",
            json!({"UpperFilters": value("MultiString", json!(["kbdclass"])), "LowerFilters": value("MultiString", crate::sensitive::MASK)}),
        );
        let result = input_configuration(&input("pcscope.desktop.input", "LocalMachine/Registry64/Class/{4d36e96b-e325-11ce-bfc1-08002be10318}", payload), &context());
        assert_eq!(field_of(&result, "upper_filters").value, Some(json!(["kbdclass"])));
        assert_eq!(field_of(&result, "lower_filters").status, ValueStatus::Masked, "a display mask is not a value to compare");
        assert_eq!(field_of(&result, "layout_file").status, ValueStatus::NotProvided);
    }

    #[test]
    fn the_input_source_keeps_the_users_hive_and_the_machines_apart() {
        let user = registry_item("CurrentUser", "Registry64", "Keyboard Layout", json!({"": value("String", "00000409"), "1": value("String", "0000040c")}));
        let machine = registry_item("LocalMachine", "Registry64", "Keyboard Layout", json!({}));
        let user = input_configuration(&input("pcscope.desktop.input", "user", user), &context());
        let machine = input_configuration(&input("pcscope.desktop.input", "machine", machine), &context());
        // The same path under the two hives is two configurations, not one.
        assert_ne!(key_of(&user.observations[0].identity), key_of(&machine.observations[0].identity));
        assert!(user.observations[0].identity.scope.contains("S-1-5-18"), "{}", user.observations[0].identity.scope);
        assert!(!machine.observations[0].identity.scope.contains("S-1-5-18"), "{}", machine.observations[0].identity.scope);
        // The preload value names this key holds are collected and no declared field names them.
        assert_eq!(field_of(&user, "layout_file").status, ValueStatus::NotProvided);
    }

    // ---------- shell configuration ----------

    #[test]
    fn both_registry_sources_describe_one_type_and_one_key_is_one_object() {
        let key = "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced";
        let values = json!({"Hidden": value("DWord", 1), "HideFileExt": value("DWord", 0), "TaskbarAl": value("DWord", 1)});
        let from_settings = shell_configuration(
            &EvidenceInput { source: "pcscope.desktop.shell-settings".into(), ..input("pcscope.desktop.shell-settings", "CurrentUser/Registry64/Advanced", registry_item("CurrentUser", "Registry64", key, values.clone())) },
            &context(),
        );
        let from_clipboard = shell_configuration(
            &EvidenceInput { source: "pcscope.desktop.clipboard".into(), ..input("pcscope.desktop.clipboard", "CurrentUser/Registry64/Advanced", registry_item("CurrentUser", "Registry64", key, values)) },
            &context(),
        );
        assert_eq!(from_settings.observations.len(), 1, "a key is one object, not one object per value");
        assert_eq!(from_settings.observations[0].identity.type_id, "desktop.shell_configuration");
        assert_eq!(key_of(&from_settings.observations[0].identity), key_of(&from_clipboard.observations[0].identity));
        // The key's own name is the object's name, and its table is the value the type declares.
        assert_eq!(field_of(&from_settings, "name").value, Some(json!("Advanced")));
        assert_eq!(field_of(&from_settings, "value").value, Some(json!({"Hidden": {"kind": "DWord", "value": 1}, "HideFileExt": {"kind": "DWord", "value": 0}, "TaskbarAl": {"kind": "DWord", "value": 1}})));
        assert_eq!(field_of(&from_settings, "value").status, ValueStatus::Known);
    }

    #[test]
    fn the_shell_settings_source_keeps_the_users_own_scope() {
        let payload = registry_item("CurrentUser", "Registry64", "Control Panel\\Desktop", json!({"Wallpaper": value("String", "C:\\示例\\壁纸.jpg")}));
        let result = shell_configuration(&input("pcscope.desktop.shell-settings", "CurrentUser/Registry64/Desktop", payload), &context());
        // Every key this source reads is the current user's, so the scope handed in is the one the
        // object belongs to — the collecting user's, user segment intact.
        assert!(result.observations[0].identity.scope.contains("S-1-5-18"), "{}", result.observations[0].identity.scope);
        assert_eq!(field_of(&result, "name").value, Some(json!("Desktop")));
        assert_eq!(field_of(&result, "value").value, Some(json!({"Wallpaper": {"kind": "String", "value": "C:\\示例\\壁纸.jpg"}})));
    }

    #[test]
    fn a_clipboard_object_from_the_machine_policy_key_is_scoped_to_the_installation() {
        let payload = registry_item("LocalMachine", "Registry64", "Software\\Policies\\Microsoft\\Windows\\System", json!({"AllowClipboardHistory": value("DWord", 0), "AllowCrossDeviceClipboard": value("DWord", 0)}));
        let result = shell_configuration(&input("pcscope.desktop.clipboard", "LocalMachine/Registry64/System", payload), &context());
        assert!(!result.observations[0].identity.scope.contains("S-1-5-18"), "{}", result.observations[0].identity.scope);
        // Two users collecting the same machine policy get the same object; their own keys do not.
        let again = shell_configuration(&input("pcscope.desktop.clipboard", "LocalMachine/Registry64/System", registry_item("LocalMachine", "Registry64", "Software\\Policies\\Microsoft\\Windows\\System", json!({}))), &context_for("machine:M/user:S-1-5-21-9/schema:1/context:/shard:0"));
        assert_eq!(key_of(&result.observations[0].identity), key_of(&again.observations[0].identity));
    }

    #[test]
    fn a_shell_configuration_item_that_is_not_a_registry_key_is_not_an_object() {
        let result = shell_configuration(&input("pcscope.desktop.clipboard", "item", json!("not a key")), &context());
        assert!(matches!(result.outcome, AdapterOutcome::NotAnObject { .. }), "{:?}", result.outcome);
        assert!(result.observations.is_empty());
    }

    // ---------- shortcuts ----------

    #[test]
    fn a_shortcut_states_no_volume_identity_and_no_file_id() {
        let payload = json!({"path": "C:\\示例\\桌面\\示例快捷方式.lnk", "target": "C:\\示例\\程序\\示例程序.exe", "working_directory": "C:\\示例\\程序", "arguments": "--示例参数", "icon": "C:\\示例\\程序\\示例程序.exe,0", "hotkey": "Ctrl+Alt+E", "basis": "existing shortcut metadata; target never executed"});
        let result = shortcuts(&input("pcscope.desktop.shortcuts", "c:\\示例\\桌面\\示例快捷方式.lnk", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "desktop.shortcut");
        // The type's strategy is `file_identity`, and the collector emits neither key.
        assert_eq!(identity.missing, vec!["volume_identity".to_owned(), "file_id".to_owned()]);
        assert!(identity.fields.iter().all(|field| field.key == "scope"), "neither missing key is given a value: {:?}", identity.fields);
        // The path is a declared field of the type and is mapped as one; it is not the identity.
        assert_eq!(field_of(&result, "path").value, Some(json!("C:\\示例\\桌面\\示例快捷方式.lnk")));
        assert_eq!(field_of(&result, "path").field_kind, FieldKind::Identity);
        assert_eq!(field_of(&result, "target").value, Some(json!("C:\\示例\\程序\\示例程序.exe")));
        assert_eq!(field_of(&result, "arguments").value, Some(json!("--示例参数")));
        assert_eq!(field_of(&result, "hotkey").value, Some(json!("Ctrl+Alt+E")));
        // The walk states that it never executes the target, and no action is invented from it.
        assert!(result.actions.is_empty());
    }

    #[test]
    fn a_shortcut_the_walk_did_not_parse_carries_only_what_it_stated() {
        // A reparse point: the walk states the target was not followed outside its scan policy, so
        // the item holds the path and the attributes and nothing else.
        let payload = json!({"path": "C:\\示例\\桌面\\示例链接", "attributes": "Directory, ReparsePoint", "collection": {"reparse_target_followed": false, "reason": "reparse_target_outside_scan_policy"}});
        let result = shortcuts(&input("pcscope.desktop.shortcuts", "c:\\示例\\桌面\\示例链接", payload), &context());
        assert_eq!(field_of(&result, "path").value, Some(json!("C:\\示例\\桌面\\示例链接")));
        for field_id in ["target", "working_directory", "arguments", "icon", "hotkey"] {
            assert_eq!(field_of(&result, field_id).status, ValueStatus::NotProvided, "{field_id}");
        }
        // A shortcut that states its link metadata is a shortcut the user's profile holds: the scope
        // is the one handed in, user segment included.
        assert!(result.observations[0].identity.scope.contains("S-1-5-18"), "{}", result.observations[0].identity.scope);
    }

    #[test]
    fn a_shortcut_the_user_stated_as_empty_is_empty_not_absent() {
        let payload = json!({"path": "C:\\示例\\桌面\\空.lnk", "target": null, "working_directory": "", "hotkey": ""});
        let result = shortcuts(&input("pcscope.desktop.shortcuts", "c:\\示例\\桌面\\空.lnk", payload), &context());
        // A `null` target is the source saying it has none...
        assert_eq!(field_of(&result, "target").status, ValueStatus::NotProvided);
        // ...while an empty string is the value the source stated, and stays one.
        assert_eq!(field_of(&result, "working_directory").status, ValueStatus::Known);
        assert_eq!(field_of(&result, "working_directory").value, Some(json!("")));
    }

    // ---------- editor extensions ----------

    #[test]
    fn an_editor_extension_states_neither_its_editor_nor_its_id() {
        let payload = json!({"name": "tool", "publisher": "example", "version": "1.2.3", "engines": {"vscode": "^1.80.0"}, "basis": "installed manifest; does not establish activation"});
        let result = ide_extensions(&input("pcscope.apps.ide-extensions", "C:\\Users\\u\\.vscode\\extensions\\example.tool-1.2.3", payload), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "desktop.editor_extension");
        // The manifest states name, publisher, version and engines — not an editor, not an id.
        assert_eq!(identity.missing, vec!["editor".to_owned(), "extension_id".to_owned()]);
        assert!(identity.fields.iter().any(|field| field.key == "version" && field.value == "1.2.3"));
        assert!(!identity.fields.iter().any(|field| field.key == "extension_id"), "the publisher and name are not composed into an id");
        assert_eq!(field_of(&result, "name").value, Some(json!("tool")));
        assert_eq!(field_of(&result, "publisher").value, Some(json!("example")));
        assert_eq!(field_of(&result, "engines").value, Some(json!({"vscode": "^1.80.0"})));
        assert_eq!(field_of(&result, "editor").status, ValueStatus::NotProvided);
        assert_eq!(field_of(&result, "extension_id").status, ValueStatus::NotProvided);
    }

    #[test]
    fn an_editor_extension_keeps_the_profile_it_was_found_in() {
        let payload = json!({"name": "tool", "publisher": "example", "version": "1.2.3"});
        let first = ide_extensions(&input("pcscope.apps.ide-extensions", "ext", payload.clone()), &context_for("machine:M/user:S-1-5-18/schema:1/context:/shard:0"));
        let second = ide_extensions(&input("pcscope.apps.ide-extensions", "ext", payload), &context_for("machine:M/user:S-1-5-21-9/schema:1/context:/shard:0"));
        // The type's scope is the editor plus the user's profile, so two profiles are two objects.
        assert_ne!(key_of(&first.observations[0].identity), key_of(&second.observations[0].identity));
    }

    #[test]
    fn material_that_is_not_a_manifest_reading_is_not_an_object() {
        let result = ide_extensions(&input("pcscope.apps.ide-extensions", "ext", json!({"basis": "no manifest was read"})), &context());
        assert!(matches!(result.outcome, AdapterOutcome::NotAnObject { .. }), "{:?}", result.outcome);
        assert!(result.observations.is_empty());
    }

    // ---------- sync accounts ----------

    #[test]
    fn a_sync_account_is_keyed_by_the_identifier_the_material_states() {
        let user = registry_item("CurrentUser", "Registry64", "Software\\Microsoft\\OneDrive\\Accounts\\Personal", json!({"AccountName": value("String", "user@example.test"), "DisplayName": value("String", "示例账户"), "UserEmail": value("String", "user@example.test"), "TenantId": value("String", "{00000000-0000-0000-0000-000000000001}")}));
        let result = sync_accounts(&input("pcscope.apps.onedrive-registration", "CurrentUser/Registry64/Accounts/Personal", user), &context());
        let identity = &result.observations[0].identity;
        assert_eq!(identity.type_id, "desktop.sync_account");
        assert!(identity.missing.is_empty(), "{:?}", identity.missing);
        assert!(identity.fields.iter().any(|field| field.key == "native_id" && field.value == "user@example.test"));
        // The account's own name, and the account it identifies, are the one value the source stated.
        assert_eq!(field_of(&result, "account").value, Some(json!("user@example.test")));
        assert_eq!(field_of(&result, "provider").value, Some(json!("OneDrive")));
        assert_eq!(field_of(&result, "mount_point").status, ValueStatus::NotProvided);
        assert_eq!(field_of(&result, "policy").status, ValueStatus::NotProvided, "this shape states no policy");

        // The sync engine's own identifier is the namespace it publishes, and it names the provider
        // as the path segment it was read under.
        let provider = registry_item("CurrentUser", "Registry64", "Software\\SyncEngines\\Providers\\OneDrive\\示例", json!({"NamespaceID": value("String", "{00000000-0000-0000-0000-000000000002}"), "MountPoint": value("String", "C:\\Users\\u\\OneDrive"), "DisplayName": value("String", "OneDrive - 示例")}));
        let result = sync_accounts(&input("pcscope.apps.onedrive-registration", "CurrentUser/Registry64/Providers/OneDrive/示例", provider), &context());
        let identity = &result.observations[0].identity;
        assert!(identity.fields.iter().any(|field| field.key == "native_id" && field.value == "{00000000-0000-0000-0000-000000000002}"));
        assert_eq!(field_of(&result, "provider").value, Some(json!("OneDrive")));
        assert_eq!(field_of(&result, "mount_point").value, Some(json!("C:\\Users\\u\\OneDrive")));
        // This shape states its account only as a display name, which is not the account's identity.
        assert_eq!(field_of(&result, "account").status, ValueStatus::NotProvided);
    }

    #[test]
    fn a_sync_registration_without_an_identifier_has_a_provisional_identity() {
        let payload = registry_item("CurrentUser", "Registry64", "Software\\Microsoft\\OneDrive\\Accounts\\Personal", json!({"DisplayName": value("String", "示例账户")}));
        let result = sync_accounts(&input("pcscope.apps.onedrive-registration", "CurrentUser/Registry64/Accounts/Personal", payload), &context());
        // The type forbids substituting the display name for the identifier, and none is invented.
        assert_eq!(result.observations[0].identity.missing, vec!["native_id".to_owned()]);
        assert_eq!(field_of(&result, "account").status, ValueStatus::NotProvided);
        assert!(result.observations[0].identity.fields.iter().all(|field| !field.value.contains("示例账户")));
        // The user's keys keep the scope they were handed.
        assert!(result.observations[0].identity.scope.contains("S-1-5-18"), "{}", result.observations[0].identity.scope);
    }

    #[test]
    fn the_machine_policy_key_states_the_policy_and_the_container_keys_register_nothing() {
        let policy = registry_item("LocalMachine", "Registry64", "Software\\Policies\\Microsoft\\OneDrive", json!({"DisableFileSyncNGSC": value("DWord", 0), "FilesOnDemandEnabled": value("DWord", 1)}));
        let result = sync_accounts(&input("pcscope.apps.onedrive-registration", "LocalMachine/Registry64/Policies/OneDrive", policy), &context());
        assert_eq!(field_of(&result, "policy").status, ValueStatus::Known);
        assert_eq!(field_of(&result, "policy").value, Some(json!({"DisableFileSyncNGSC": {"kind": "DWord", "value": 0}, "FilesOnDemandEnabled": {"kind": "DWord", "value": 1}})));
        assert_eq!(field_of(&result, "provider").value, Some(json!("OneDrive")));
        assert!(!result.observations[0].identity.scope.contains("S-1-5-18"), "the machine key belongs to the installation");

        // The containers the collector also emits are the lists, not an entry.
        for path in ["Software\\Microsoft\\OneDrive\\Accounts", "Software\\SyncEngines\\Providers\\OneDrive"] {
            let holder = registry_item("CurrentUser", "Registry64", path, json!({}));
            let result = sync_accounts(&input("pcscope.apps.onedrive-registration", "container", holder), &context());
            assert!(matches!(result.outcome, AdapterOutcome::Skipped { .. }), "{path}: {:?}", result.outcome);
            assert!(result.observations.is_empty(), "no object is invented for it");
        }

        let not_a_key = sync_accounts(&input("pcscope.apps.onedrive-registration", "item", json!("not a key")), &context());
        assert!(matches!(not_a_key.outcome, AdapterOutcome::NotAnObject { .. }), "{:?}", not_a_key.outcome);
    }

    // ---------- determinism ----------

    #[test]
    fn the_same_material_produces_the_same_candidates_twice() {
        let context = context();
        for (source, item_id, payload) in samples() {
            let once: Vec<String> = (spec_for(source).expect("declared").build)(&input(source, item_id, payload.clone()), &context)
                .observations
                .iter()
                .map(|candidate| key_of(&candidate.identity))
                .collect();
            let twice: Vec<String> = (spec_for(source).expect("declared").build)(&input(source, item_id, payload), &context)
                .observations
                .iter()
                .map(|candidate| key_of(&candidate.identity))
                .collect();
            assert_eq!(once, twice, "{source}: replaying the same material must decide the same thing");
            assert_eq!(once.len(), 1, "{source}");
        }
    }
}
