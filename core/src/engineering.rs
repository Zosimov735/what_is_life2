//! Canonical engineering-memory authority records.
//!
//! Generator design, opening assembly, lineage, and qualification evidence are
//! separate records. Their ids address canonical definition bytes; browser
//! names, tags, timestamps, and thumbnails are intentionally absent.

use crate::fault::Fault;
use crate::json::{hex_bytes, Json, Obj};
use crate::read;
use crate::sha256;
use crate::state::{
    AssemblyComponentDraft, AssemblyCurrentDraft, AssemblyDraft, AssemblyFormDraft,
    AssemblyMaterialDraft, AssemblyTemplate, GeneratorSpec, ASSEMBLY_OWNED_FIELDS, REGIME_IDS,
};

pub const ENGINEERING_RECORD_VERSION: u8 = 2;
pub const ENGINEERING_TRANSITION_VERSION: u8 = 5;

fn definition_id(definition: &str) -> String {
    hex_bytes(&sha256::digest(definition.as_bytes()))
}

fn canonical_definition(value: &Json) -> Result<String, Fault> {
    let mut definition = String::new();
    crate::json::write_value(&mut definition, value).map_err(|_| Fault::field("definition"))?;
    Ok(definition)
}

fn exact_record<'a>(value: &'a Json, key: &str, id_key: &str) -> Result<(&'a Json, &'a str), Fault> {
    let record = read::map(value, key)?;
    read::exact_keys(record, key, &["definition", id_key])?;
    let definition = read::map(record, "definition")?;
    let record_id = read::hex(record, id_key, 64)?;
    Ok((definition, record_id))
}

fn valid_contract_id(value: &str) -> bool {
    !value.is_empty()
        && value.starts_with(|first: char| first.is_ascii_lowercase())
        && value
            .chars()
            .all(|held| held.is_ascii_lowercase() || held.is_ascii_digit() || held == '_')
}

fn read_attempt_id(value: &Json, key: &str) -> Result<String, Fault> {
    let found = read::text(value, key)?;
    if found.len() == 16
        && found
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(found.to_string())
    } else {
        Err(Fault::field(key))
    }
}

fn read_nullable_hex(value: &Json, key: &str) -> Result<Option<String>, Fault> {
    match read::at(value, key)? {
        Json::Null => Ok(None),
        Json::Text(_) => Ok(Some(read::hex(value, key, 64)?.to_string())),
        _ => Err(Fault::field(key)),
    }
}

fn valid_address(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b':' | b'.'))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineeringTransitionKind {
    RestartAssembly,
    RevertGenerator,
    FullContractReset,
}

impl EngineeringTransitionKind {
    pub fn name(self) -> &'static str {
        match self {
            EngineeringTransitionKind::RestartAssembly => "restart_assembly",
            EngineeringTransitionKind::RevertGenerator => "revert_generator",
            EngineeringTransitionKind::FullContractReset => "full_contract_reset",
        }
    }

    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &["restart_assembly", "revert_generator", "full_contract_reset"],
        )? {
            0 => EngineeringTransitionKind::RestartAssembly,
            1 => EngineeringTransitionKind::RevertGenerator,
            _ => EngineeringTransitionKind::FullContractReset,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineeringTransitionSourceKind {
    CurrentCommitted,
    GeneratorRecord,
    AuthoredContractOpening,
}

impl EngineeringTransitionSourceKind {
    pub fn name(self) -> &'static str {
        match self {
            EngineeringTransitionSourceKind::CurrentCommitted => "current_committed",
            EngineeringTransitionSourceKind::GeneratorRecord => "generator_record",
            EngineeringTransitionSourceKind::AuthoredContractOpening => {
                "authored_contract_opening"
            }
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &[
                "current_committed",
                "generator_record",
                "authored_contract_opening",
            ],
        )? {
            0 => EngineeringTransitionSourceKind::CurrentCommitted,
            1 => EngineeringTransitionSourceKind::GeneratorRecord,
            _ => EngineeringTransitionSourceKind::AuthoredContractOpening,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineeringTransitionSource {
    pub kind: EngineeringTransitionSourceKind,
    pub source_id: Option<String>,
}

impl EngineeringTransitionSource {
    pub fn current_committed() -> Self {
        Self {
            kind: EngineeringTransitionSourceKind::CurrentCommitted,
            source_id: None,
        }
    }

    pub fn generator_record(source_id: &str) -> Result<Self, Fault> {
        Self::new(
            EngineeringTransitionSourceKind::GeneratorRecord,
            Some(source_id.to_string()),
        )
    }

    pub fn authored_contract_opening(content_hash: &str) -> Result<Self, Fault> {
        Self::new(
            EngineeringTransitionSourceKind::AuthoredContractOpening,
            Some(content_hash.to_string()),
        )
    }

    fn new(
        kind: EngineeringTransitionSourceKind,
        source_id: Option<String>,
    ) -> Result<Self, Fault> {
        let valid = match (&kind, &source_id) {
            (EngineeringTransitionSourceKind::CurrentCommitted, None) => true,
            (
                EngineeringTransitionSourceKind::GeneratorRecord
                | EngineeringTransitionSourceKind::AuthoredContractOpening,
                Some(id),
            ) => crate::json::is_hex(id, 64),
            _ => false,
        };
        if !valid {
            return Err(Fault::field("transition_source"));
        }
        Ok(Self { kind, source_id })
    }

    pub fn supports(&self, operation: EngineeringTransitionKind) -> bool {
        matches!(
            (operation, self.kind),
            (
                EngineeringTransitionKind::RestartAssembly,
                EngineeringTransitionSourceKind::CurrentCommitted
            ) | (
                EngineeringTransitionKind::RevertGenerator,
                EngineeringTransitionSourceKind::GeneratorRecord
            ) | (
                EngineeringTransitionKind::FullContractReset,
                EngineeringTransitionSourceKind::AuthoredContractOpening
            )
        )
    }

    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(found, key, &["kind", "source_id", "version"])?;
        if read::int(found, "version", 1, 1)? != 1 {
            return Err(Fault::field(key));
        }
        let source_id = read_nullable_hex(found, "source_id")?;
        Self::new(
            EngineeringTransitionSourceKind::read(found, "kind")?,
            source_id,
        )
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("kind", self.kind.name());
        match &self.source_id {
            Some(id) => object.text("source_id", id),
            None => object.null("source_id"),
        };
        object.int("version", 1);
        object.end();
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineeringIdentityDisposition {
    Retained,
    Restored,
    Recreated,
    Detached,
}

impl EngineeringIdentityDisposition {
    pub fn name(self) -> &'static str {
        match self {
            EngineeringIdentityDisposition::Retained => "retained",
            EngineeringIdentityDisposition::Restored => "restored",
            EngineeringIdentityDisposition::Recreated => "recreated",
            EngineeringIdentityDisposition::Detached => "detached",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &["retained", "restored", "recreated", "detached"],
        )? {
            0 => EngineeringIdentityDisposition::Retained,
            1 => EngineeringIdentityDisposition::Restored,
            2 => EngineeringIdentityDisposition::Recreated,
            _ => EngineeringIdentityDisposition::Detached,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineeringIdentityKind {
    Attempt,
    Branch,
    Generator,
    Assembly,
    QualificationRequest,
    QualificationResult,
    Blueprint,
}

impl EngineeringIdentityKind {
    pub fn name(self) -> &'static str {
        match self {
            EngineeringIdentityKind::Attempt => "attempt",
            EngineeringIdentityKind::Branch => "branch",
            EngineeringIdentityKind::Generator => "generator",
            EngineeringIdentityKind::Assembly => "assembly",
            EngineeringIdentityKind::QualificationRequest => "qualification_request",
            EngineeringIdentityKind::QualificationResult => "qualification_result",
            EngineeringIdentityKind::Blueprint => "blueprint",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &[
                "attempt",
                "branch",
                "generator",
                "assembly",
                "qualification_request",
                "qualification_result",
                "blueprint",
            ],
        )? {
            0 => EngineeringIdentityKind::Attempt,
            1 => EngineeringIdentityKind::Branch,
            2 => EngineeringIdentityKind::Generator,
            3 => EngineeringIdentityKind::Assembly,
            4 => EngineeringIdentityKind::QualificationRequest,
            5 => EngineeringIdentityKind::QualificationResult,
            _ => EngineeringIdentityKind::Blueprint,
        })
    }

    fn valid_identity(self, identity: &str) -> bool {
        match self {
            EngineeringIdentityKind::Attempt => {
                identity.len() == 16 && identity.bytes().all(|byte| byte.is_ascii_hexdigit())
            }
            _ => crate::json::is_hex(identity, 64),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineeringTransitionIdentity {
    pub disposition: EngineeringIdentityDisposition,
    pub identity: String,
    pub kind: EngineeringIdentityKind,
}

impl EngineeringTransitionIdentity {
    pub fn new(
        disposition: EngineeringIdentityDisposition,
        kind: EngineeringIdentityKind,
        identity: &str,
    ) -> Result<Self, Fault> {
        if !kind.valid_identity(identity) {
            return Err(Fault::field("transition_identity"));
        }
        Ok(Self {
            disposition,
            identity: identity.to_string(),
            kind,
        })
    }

    fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(
            value,
            "identities",
            &["disposition", "identity", "kind", "version"],
        )?;
        if read::int(value, "version", 1, 1)? != 1 {
            return Err(Fault::field("identities"));
        }
        Self::new(
            EngineeringIdentityDisposition::read(value, "disposition")?,
            EngineeringIdentityKind::read(value, "kind")?,
            read::text(value, "identity")?,
        )
    }

    fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("disposition", self.disposition.name());
        object.text("identity", &self.identity);
        object.text("kind", self.kind.name());
        object.int("version", 1);
        object.end();
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineeringTransitionRegisterKind {
    LivePositions,
    StoredCharge,
    PolicyTimers,
    ControllerState,
    EventWindow,
    ProvisionalCriteria,
}

impl EngineeringTransitionRegisterKind {
    const ALL: [Self; 6] = [
        Self::LivePositions,
        Self::StoredCharge,
        Self::PolicyTimers,
        Self::ControllerState,
        Self::EventWindow,
        Self::ProvisionalCriteria,
    ];

    fn ordinal(self) -> u8 {
        match self {
            Self::LivePositions => 0,
            Self::StoredCharge => 1,
            Self::PolicyTimers => 2,
            Self::ControllerState => 3,
            Self::EventWindow => 4,
            Self::ProvisionalCriteria => 5,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            EngineeringTransitionRegisterKind::LivePositions => "live_positions",
            EngineeringTransitionRegisterKind::StoredCharge => "stored_charge",
            EngineeringTransitionRegisterKind::PolicyTimers => "policy_timers",
            EngineeringTransitionRegisterKind::ControllerState => "controller_state",
            EngineeringTransitionRegisterKind::EventWindow => "event_window",
            EngineeringTransitionRegisterKind::ProvisionalCriteria => "provisional_criteria",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &[
                "live_positions",
                "stored_charge",
                "policy_timers",
                "controller_state",
                "event_window",
                "provisional_criteria",
            ],
        )? {
            0 => EngineeringTransitionRegisterKind::LivePositions,
            1 => EngineeringTransitionRegisterKind::StoredCharge,
            2 => EngineeringTransitionRegisterKind::PolicyTimers,
            3 => EngineeringTransitionRegisterKind::ControllerState,
            4 => EngineeringTransitionRegisterKind::EventWindow,
            _ => EngineeringTransitionRegisterKind::ProvisionalCriteria,
        })
    }
}

fn complete_transition_registers(
    registers: &[EngineeringTransitionRegisterConsequence],
) -> bool {
    registers.len() == EngineeringTransitionRegisterKind::ALL.len()
        && registers
            .iter()
            .zip(EngineeringTransitionRegisterKind::ALL)
            .all(|(register, expected)| register.kind == expected)
}

/// One exact runtime register that leaves the parent Commission branch and is
/// reconstructed for the child. Addresses localize the affected authority;
/// digests bind the complete canonical register before and after reconstruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineeringTransitionRegisterConsequence {
    pub addresses: Vec<String>,
    pub after_digest: String,
    pub before_digest: String,
    pub kind: EngineeringTransitionRegisterKind,
}

impl EngineeringTransitionRegisterConsequence {
    pub fn new(
        kind: EngineeringTransitionRegisterKind,
        before_digest: &str,
        after_digest: &str,
        mut addresses: Vec<String>,
    ) -> Result<Self, Fault> {
        if !crate::json::is_hex(before_digest, 64)
            || !crate::json::is_hex(after_digest, 64)
            || addresses.len() > 512
            || addresses.iter().any(|address| !valid_address(address))
        {
            return Err(Fault::field("transition_register"));
        }
        addresses.sort();
        addresses.dedup();
        Ok(Self {
            addresses,
            after_digest: after_digest.to_string(),
            before_digest: before_digest.to_string(),
            kind,
        })
    }

    fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(
            value,
            "registers",
            &[
                "addresses",
                "after_digest",
                "after_disposition",
                "before_digest",
                "before_disposition",
                "kind",
                "version",
            ],
        )?;
        if read::int(value, "version", 1, 1)? != 1
            || read::one_of(value, "after_disposition", &["recreated"])? != 0
            || read::one_of(value, "before_disposition", &["detached"])? != 0
        {
            return Err(Fault::field("registers"));
        }
        let mut addresses = Vec::new();
        for address in read::list(value, "addresses", 512)? {
            match address {
                Json::Text(address) if valid_address(address) => addresses.push(address.clone()),
                _ => return Err(Fault::field("addresses")),
            }
        }
        if !read::ascending(&addresses) {
            return Err(Fault::field("addresses"));
        }
        Self::new(
            EngineeringTransitionRegisterKind::read(value, "kind")?,
            read::hex(value, "before_digest", 64)?,
            read::hex(value, "after_digest", 64)?,
            addresses,
        )
    }

    fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        {
            let mut addresses = object.list("addresses");
            for address in &self.addresses {
                addresses.text(address);
            }
            addresses.end();
        }
        object.text("after_digest", &self.after_digest);
        object.text("after_disposition", "recreated");
        object.text("before_digest", &self.before_digest);
        object.text("before_disposition", "detached");
        object.text("kind", self.kind.name());
        object.int("version", 1);
        object.end();
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineeringCompatibilityDisposition {
    HardIncompatibility,
    AssemblyAdaptation,
    GeneratorEditRequired,
}

impl EngineeringCompatibilityDisposition {
    pub fn name(self) -> &'static str {
        match self {
            EngineeringCompatibilityDisposition::HardIncompatibility => "hard_incompatibility",
            EngineeringCompatibilityDisposition::AssemblyAdaptation => "assembly_adaptation",
            EngineeringCompatibilityDisposition::GeneratorEditRequired => {
                "generator_edit_required"
            }
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &[
                "hard_incompatibility",
                "assembly_adaptation",
                "generator_edit_required",
            ],
        )? {
            0 => EngineeringCompatibilityDisposition::HardIncompatibility,
            1 => EngineeringCompatibilityDisposition::AssemblyAdaptation,
            _ => EngineeringCompatibilityDisposition::GeneratorEditRequired,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineeringCompatibilityCode {
    MissingHardware,
    UnsupportedAction,
    UnsupportedCondition,
    InvalidAddress,
    InvalidRouteOwnership,
    MissingMaterial,
    RegimeIncompatibleAssembly,
    GeneratorEditRequired,
}

impl EngineeringCompatibilityCode {
    pub fn name(self) -> &'static str {
        match self {
            EngineeringCompatibilityCode::MissingHardware => "missing_hardware",
            EngineeringCompatibilityCode::UnsupportedAction => "unsupported_action",
            EngineeringCompatibilityCode::UnsupportedCondition => "unsupported_condition",
            EngineeringCompatibilityCode::InvalidAddress => "invalid_address",
            EngineeringCompatibilityCode::InvalidRouteOwnership => "invalid_route_ownership",
            EngineeringCompatibilityCode::MissingMaterial => "missing_material",
            EngineeringCompatibilityCode::RegimeIncompatibleAssembly => {
                "regime_incompatible_assembly"
            }
            EngineeringCompatibilityCode::GeneratorEditRequired => "generator_edit_required",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &[
                "missing_hardware",
                "unsupported_action",
                "unsupported_condition",
                "invalid_address",
                "invalid_route_ownership",
                "missing_material",
                "regime_incompatible_assembly",
                "generator_edit_required",
            ],
        )? {
            0 => EngineeringCompatibilityCode::MissingHardware,
            1 => EngineeringCompatibilityCode::UnsupportedAction,
            2 => EngineeringCompatibilityCode::UnsupportedCondition,
            3 => EngineeringCompatibilityCode::InvalidAddress,
            4 => EngineeringCompatibilityCode::InvalidRouteOwnership,
            5 => EngineeringCompatibilityCode::MissingMaterial,
            6 => EngineeringCompatibilityCode::RegimeIncompatibleAssembly,
            _ => EngineeringCompatibilityCode::GeneratorEditRequired,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineeringTransitionCompatibilityIssue {
    pub address: Option<String>,
    pub code: EngineeringCompatibilityCode,
    pub disposition: EngineeringCompatibilityDisposition,
}

impl EngineeringTransitionCompatibilityIssue {
    pub fn new(
        address: Option<String>,
        code: EngineeringCompatibilityCode,
        disposition: EngineeringCompatibilityDisposition,
    ) -> Result<Self, Fault> {
        if address.as_ref().is_some_and(|value| !valid_address(value)) {
            return Err(Fault::field("compatibility_issue"));
        }
        Ok(Self {
            address,
            code,
            disposition,
        })
    }

    fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(
            value,
            "compatibility_issues",
            &["address", "code", "disposition", "version"],
        )?;
        if read::int(value, "version", 1, 1)? != 1 {
            return Err(Fault::field("compatibility_issues"));
        }
        let address = match read::at(value, "address")? {
            Json::Null => None,
            Json::Text(address) if valid_address(address) => Some(address.clone()),
            _ => return Err(Fault::field("compatibility_issues")),
        };
        Self::new(
            address,
            EngineeringCompatibilityCode::read(value, "code")?,
            EngineeringCompatibilityDisposition::read(value, "disposition")?,
        )
    }

    fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        match &self.address {
            Some(address) => object.text("address", address),
            None => object.null("address"),
        };
        object.text("code", self.code.name());
        object.text("disposition", self.disposition.name());
        object.int("version", 1);
        object.end();
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineeringAssemblyCompatibilityFieldKind {
    ComponentPosition,
    ComponentLayer,
    StoredCharge,
    InterfaceState,
    FormReserve,
    JunctionBlanks,
    MaterialAmount,
    MaterialPosition,
    MaterialLayer,
    CurrentActive,
    CurrentPhase,
    PhysicalCompartmentMembers,
    PhysicalCompartmentLeakage,
}

impl EngineeringAssemblyCompatibilityFieldKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::ComponentPosition => "component_position",
            Self::ComponentLayer => "component_layer",
            Self::StoredCharge => "stored_charge",
            Self::InterfaceState => "interface_state",
            Self::FormReserve => "form_reserve",
            Self::JunctionBlanks => "junction_blanks",
            Self::MaterialAmount => "material_amount",
            Self::MaterialPosition => "material_position",
            Self::MaterialLayer => "material_layer",
            Self::CurrentActive => "current_active",
            Self::CurrentPhase => "current_phase",
            Self::PhysicalCompartmentMembers => "physical_compartment_members",
            Self::PhysicalCompartmentLeakage => "physical_compartment_leakage",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &[
                "component_position",
                "component_layer",
                "stored_charge",
                "interface_state",
                "form_reserve",
                "junction_blanks",
                "material_amount",
                "material_position",
                "material_layer",
                "current_active",
                "current_phase",
                "physical_compartment_members",
                "physical_compartment_leakage",
            ],
        )? {
            0 => Self::ComponentPosition,
            1 => Self::ComponentLayer,
            2 => Self::StoredCharge,
            3 => Self::InterfaceState,
            4 => Self::FormReserve,
            5 => Self::JunctionBlanks,
            6 => Self::MaterialAmount,
            7 => Self::MaterialPosition,
            8 => Self::MaterialLayer,
            9 => Self::CurrentActive,
            10 => Self::CurrentPhase,
            11 => Self::PhysicalCompartmentMembers,
            _ => Self::PhysicalCompartmentLeakage,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineeringAssemblyCompatibilityDisposition {
    RetainedUnchanged,
    RetainedByAddress,
    AdaptationRequired,
    HardRefusal,
}

impl EngineeringAssemblyCompatibilityDisposition {
    pub fn name(self) -> &'static str {
        match self {
            Self::RetainedUnchanged => "retained_unchanged",
            Self::RetainedByAddress => "retained_by_address",
            Self::AdaptationRequired => "adaptation_required",
            Self::HardRefusal => "hard_refusal",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &[
                "retained_unchanged",
                "retained_by_address",
                "adaptation_required",
                "hard_refusal",
            ],
        )? {
            0 => Self::RetainedUnchanged,
            1 => Self::RetainedByAddress,
            2 => Self::AdaptationRequired,
            _ => Self::HardRefusal,
        })
    }
}

/** One assembly-owned opening field classified against a selected generator. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineeringAssemblyCompatibilityField {
    pub address: String,
    pub after_digest: String,
    pub before_digest: String,
    pub disposition: EngineeringAssemblyCompatibilityDisposition,
    pub field: EngineeringAssemblyCompatibilityFieldKind,
    pub issue_code: Option<EngineeringCompatibilityCode>,
}

impl EngineeringAssemblyCompatibilityField {
    pub fn new(
        address: &str,
        field: EngineeringAssemblyCompatibilityFieldKind,
        disposition: EngineeringAssemblyCompatibilityDisposition,
        before_value: &str,
        after_value: &str,
        issue_code: Option<EngineeringCompatibilityCode>,
    ) -> Result<Self, Fault> {
        if !valid_address(address) {
            return Err(Fault::field("compatibility_fields"));
        }
        Ok(Self {
            address: address.to_string(),
            after_digest: definition_id(after_value),
            before_digest: definition_id(before_value),
            disposition,
            field,
            issue_code,
        })
    }

    fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(
            value,
            "compatibility_fields",
            &[
                "address",
                "after_digest",
                "before_digest",
                "disposition",
                "field",
                "issue_code",
                "version",
            ],
        )?;
        if read::int(value, "version", 1, 1)? != 1 {
            return Err(Fault::field("compatibility_fields"));
        }
        let issue_code = match read::at(value, "issue_code")? {
            Json::Null => None,
            Json::Text(_) => Some(EngineeringCompatibilityCode::read(value, "issue_code")?),
            _ => return Err(Fault::field("compatibility_fields")),
        };
        let field = Self {
            address: read::text(value, "address")?.to_string(),
            after_digest: read::hex(value, "after_digest", 64)?.to_string(),
            before_digest: read::hex(value, "before_digest", 64)?.to_string(),
            disposition: EngineeringAssemblyCompatibilityDisposition::read(
                value,
                "disposition",
            )?,
            field: EngineeringAssemblyCompatibilityFieldKind::read(value, "field")?,
            issue_code,
        };
        if !valid_address(&field.address) {
            return Err(Fault::field("compatibility_fields"));
        }
        Ok(field)
    }

    fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("address", &self.address);
        object.text("after_digest", &self.after_digest);
        object.text("before_digest", &self.before_digest);
        object.text("disposition", self.disposition.name());
        object.text("field", self.field.name());
        match self.issue_code {
            Some(code) => object.text("issue_code", code.name()),
            None => object.null("issue_code"),
        };
        object.int("version", 1);
        object.end();
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineeringTransitionRefusalCode {
    WrongRunKind,
    WrongLifecycle,
    StaleContract,
    StaleAttempt,
    StaleBranch,
    StaleGenerator,
    StaleAssembly,
    StalePreview,
    SourceUnavailable,
    SourceCorrupt,
    IncompatibleAssembly,
    ReconstructionFailed,
    QualificationFrozen,
    UnsupportedOperation,
}

impl EngineeringTransitionRefusalCode {
    pub fn name(self) -> &'static str {
        match self {
            EngineeringTransitionRefusalCode::WrongRunKind => "wrong_run_kind",
            EngineeringTransitionRefusalCode::WrongLifecycle => "wrong_lifecycle",
            EngineeringTransitionRefusalCode::StaleContract => "stale_contract",
            EngineeringTransitionRefusalCode::StaleAttempt => "stale_attempt",
            EngineeringTransitionRefusalCode::StaleBranch => "stale_branch",
            EngineeringTransitionRefusalCode::StaleGenerator => "stale_generator",
            EngineeringTransitionRefusalCode::StaleAssembly => "stale_assembly",
            EngineeringTransitionRefusalCode::StalePreview => "stale_preview",
            EngineeringTransitionRefusalCode::SourceUnavailable => "source_unavailable",
            EngineeringTransitionRefusalCode::SourceCorrupt => "source_corrupt",
            EngineeringTransitionRefusalCode::IncompatibleAssembly => "incompatible_assembly",
            EngineeringTransitionRefusalCode::ReconstructionFailed => "reconstruction_failed",
            EngineeringTransitionRefusalCode::QualificationFrozen => "qualification_frozen",
            EngineeringTransitionRefusalCode::UnsupportedOperation => "unsupported_operation",
        }
    }

    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &[
                "wrong_run_kind",
                "wrong_lifecycle",
                "stale_contract",
                "stale_attempt",
                "stale_branch",
                "stale_generator",
                "stale_assembly",
                "stale_preview",
                "source_unavailable",
                "source_corrupt",
                "incompatible_assembly",
                "reconstruction_failed",
                "qualification_frozen",
                "unsupported_operation",
            ],
        )? {
            0 => EngineeringTransitionRefusalCode::WrongRunKind,
            1 => EngineeringTransitionRefusalCode::WrongLifecycle,
            2 => EngineeringTransitionRefusalCode::StaleContract,
            3 => EngineeringTransitionRefusalCode::StaleAttempt,
            4 => EngineeringTransitionRefusalCode::StaleBranch,
            5 => EngineeringTransitionRefusalCode::StaleGenerator,
            6 => EngineeringTransitionRefusalCode::StaleAssembly,
            7 => EngineeringTransitionRefusalCode::StalePreview,
            8 => EngineeringTransitionRefusalCode::SourceUnavailable,
            9 => EngineeringTransitionRefusalCode::SourceCorrupt,
            10 => EngineeringTransitionRefusalCode::IncompatibleAssembly,
            11 => EngineeringTransitionRefusalCode::ReconstructionFailed,
            12 => EngineeringTransitionRefusalCode::QualificationFrozen,
            _ => EngineeringTransitionRefusalCode::UnsupportedOperation,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineeringTransitionRefusal {
    pub code: EngineeringTransitionRefusalCode,
    pub field: Option<String>,
    pub operation: EngineeringTransitionKind,
}

impl EngineeringTransitionRefusal {
    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("code", self.code.name());
        match &self.field {
            Some(field) => object.text("field", field),
            None => object.null("field"),
        };
        object.text("operation", self.operation.name());
        object.text("status", "refused");
        object.int("version", 1);
        object.end();
        out
    }

    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(
            found,
            key,
            &["code", "field", "operation", "status", "version"],
        )?;
        if read::one_of(found, "status", &["refused"])? != 0
            || read::int(found, "version", 1, 1)? != 1
        {
            return Err(Fault::field(key));
        }
        let field = match read::at(found, "field")? {
            Json::Null => None,
            Json::Text(field) if valid_address(field) => Some(field.clone()),
            _ => return Err(Fault::field(key)),
        };
        Ok(Self {
            code: EngineeringTransitionRefusalCode::read(found, "code")?,
            field,
            operation: EngineeringTransitionKind::read(found, "operation")?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineeringTransitionGuard {
    pub assembly_hash: String,
    pub attempt_id: String,
    pub branch_id: String,
    pub branch_nonce: u32,
    pub content_hash: String,
    pub contract_id: String,
    pub embodied_hash: String,
    pub generator_hash: String,
    pub lifecycle: String,
    pub run_kind: String,
    pub scenario_hash: String,
}

impl EngineeringTransitionGuard {
    pub fn new(
        assembly_hash: &str,
        attempt_id: &str,
        branch_id: &str,
        branch_nonce: u32,
        content_hash: &str,
        contract_id: &str,
        embodied_hash: &str,
        generator_hash: &str,
        lifecycle: &str,
        run_kind: &str,
        scenario_hash: &str,
    ) -> Result<Self, Fault> {
        if !crate::json::is_hex(assembly_hash, 64)
            || !crate::json::is_hex(branch_id, 64)
            || !crate::json::is_hex(content_hash, 64)
            || !crate::json::is_hex(embodied_hash, 64)
            || !crate::json::is_hex(generator_hash, 64)
            || !crate::json::is_hex(scenario_hash, 64)
            || attempt_id.len() != 16
            || !attempt_id.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !valid_contract_id(contract_id)
            || lifecycle != "still"
            || run_kind != "automation_contract"
        {
            return Err(Fault::field("transition_guard"));
        }
        Ok(Self {
            assembly_hash: assembly_hash.to_string(),
            attempt_id: attempt_id.to_string(),
            branch_id: branch_id.to_string(),
            branch_nonce,
            content_hash: content_hash.to_string(),
            contract_id: contract_id.to_string(),
            embodied_hash: embodied_hash.to_string(),
            generator_hash: generator_hash.to_string(),
            lifecycle: lifecycle.to_string(),
            run_kind: run_kind.to_string(),
            scenario_hash: scenario_hash.to_string(),
        })
    }

    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(
            found,
            key,
            &[
                "assembly_hash",
                "attempt_id",
                "branch_id",
                "branch_nonce",
                "content_hash",
                "contract_id",
                "embodied_hash",
                "generator_hash",
                "lifecycle",
                "run_kind",
                "scenario_hash",
                "version",
            ],
        )?;
        if read::int(found, "version", 2, 2)? != 2 {
            return Err(Fault::field(key));
        }
        Self::new(
            read::hex(found, "assembly_hash", 64)?,
            read::text(found, "attempt_id")?,
            read::hex(found, "branch_id", 64)?,
            read::int(found, "branch_nonce", 0, i64::from(u32::MAX))? as u32,
            read::hex(found, "content_hash", 64)?,
            read::text(found, "contract_id")?,
            read::hex(found, "embodied_hash", 64)?,
            read::hex(found, "generator_hash", 64)?,
            read::text(found, "lifecycle")?,
            read::text(found, "run_kind")?,
            read::hex(found, "scenario_hash", 64)?,
        )
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("assembly_hash", &self.assembly_hash);
        object.text("attempt_id", &self.attempt_id);
        object.text("branch_id", &self.branch_id);
        object.int("branch_nonce", i64::from(self.branch_nonce));
        object.text("content_hash", &self.content_hash);
        object.text("contract_id", &self.contract_id);
        object.text("embodied_hash", &self.embodied_hash);
        object.text("generator_hash", &self.generator_hash);
        object.text("lifecycle", &self.lifecycle);
        object.text("run_kind", &self.run_kind);
        object.text("scenario_hash", &self.scenario_hash);
        object.int("version", 2);
        object.end();
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineeringTransitionPreview {
    pub commit_allowed: bool,
    pub compatibility_fields: Vec<EngineeringAssemblyCompatibilityField>,
    pub compatibility_issues: Vec<EngineeringTransitionCompatibilityIssue>,
    pub current_regime_id: String,
    pub definition: String,
    pub guard: EngineeringTransitionGuard,
    pub identities: Vec<EngineeringTransitionIdentity>,
    pub operation: EngineeringTransitionKind,
    pub preview_id: String,
    pub reconstruction_digest: String,
    pub registers: Vec<EngineeringTransitionRegisterConsequence>,
    pub source: EngineeringTransitionSource,
    pub target_assembly_draft: crate::state::AssemblyDraft,
    pub target_assembly_hash: String,
    pub target_generator_hash: String,
    pub target_regime_id: String,
    pub target_scenario_hash: String,
}

impl EngineeringTransitionPreview {
    pub fn new(
        operation: EngineeringTransitionKind,
        source: EngineeringTransitionSource,
        guard: EngineeringTransitionGuard,
        current_regime_id: &str,
        target_assembly_draft: &crate::state::AssemblyDraft,
        target_generator_hash: &str,
        target_assembly_hash: &str,
        target_regime_id: &str,
        target_scenario_hash: &str,
        reconstruction_digest: &str,
        mut identities: Vec<EngineeringTransitionIdentity>,
        mut registers: Vec<EngineeringTransitionRegisterConsequence>,
        mut compatibility_fields: Vec<EngineeringAssemblyCompatibilityField>,
        mut compatibility_issues: Vec<EngineeringTransitionCompatibilityIssue>,
    ) -> Result<Self, Fault> {
        if !source.supports(operation)
            || !crate::json::is_hex(target_generator_hash, 64)
            || !crate::json::is_hex(target_assembly_hash, 64)
            || !valid_address(current_regime_id)
            || !valid_address(target_regime_id)
            || !crate::json::is_hex(target_scenario_hash, 64)
            || !crate::json::is_hex(reconstruction_digest, 64)
        {
            return Err(Fault::field("transition_preview"));
        }
        identities.sort_by(|left, right| {
            (left.kind.name(), left.identity.as_str(), left.disposition.name()).cmp(&(
                right.kind.name(),
                right.identity.as_str(),
                right.disposition.name(),
            ))
        });
        compatibility_issues.sort_by(|left, right| {
            (
                left.address.as_deref().unwrap_or_default(),
                left.code.name(),
                left.disposition.name(),
            )
                .cmp(&(
                    right.address.as_deref().unwrap_or_default(),
                    right.code.name(),
                    right.disposition.name(),
                ))
        });
        compatibility_fields.sort_by(|left, right| {
            (left.address.as_str(), left.field.name()).cmp(&(
                right.address.as_str(),
                right.field.name(),
            ))
        });
        if compatibility_fields.windows(2).any(|pair| {
            pair[0].address == pair[1].address && pair[0].field == pair[1].field
        }) {
            return Err(Fault::field("compatibility_fields"));
        }
        registers.sort_by_key(|register| register.kind.ordinal());
        if !complete_transition_registers(&registers) {
            return Err(Fault::field("registers"));
        }
        let commit_allowed = compatibility_issues.is_empty()
            && compatibility_fields.iter().all(|field| {
                matches!(
                    field.disposition,
                    EngineeringAssemblyCompatibilityDisposition::RetainedUnchanged
                        | EngineeringAssemblyCompatibilityDisposition::RetainedByAddress
                )
            });
        let definition = Self::definition_written(
            operation,
            &source,
            &guard,
            current_regime_id,
            target_assembly_draft,
            target_generator_hash,
            target_assembly_hash,
            target_regime_id,
            target_scenario_hash,
            reconstruction_digest,
            &identities,
            &registers,
            commit_allowed,
            &compatibility_fields,
            &compatibility_issues,
        );
        let preview_id = definition_id(&definition);
        Ok(Self {
            commit_allowed,
            compatibility_fields,
            compatibility_issues,
            current_regime_id: current_regime_id.to_string(),
            definition,
            guard,
            identities,
            operation,
            preview_id,
            reconstruction_digest: reconstruction_digest.to_string(),
            registers,
            source,
            target_assembly_draft: target_assembly_draft.clone(),
            target_assembly_hash: target_assembly_hash.to_string(),
            target_generator_hash: target_generator_hash.to_string(),
            target_regime_id: target_regime_id.to_string(),
            target_scenario_hash: target_scenario_hash.to_string(),
        })
    }

    fn definition_written(
        operation: EngineeringTransitionKind,
        source: &EngineeringTransitionSource,
        guard: &EngineeringTransitionGuard,
        current_regime_id: &str,
        target_assembly_draft: &crate::state::AssemblyDraft,
        target_generator_hash: &str,
        target_assembly_hash: &str,
        target_regime_id: &str,
        target_scenario_hash: &str,
        reconstruction_digest: &str,
        identities: &[EngineeringTransitionIdentity],
        registers: &[EngineeringTransitionRegisterConsequence],
        commit_allowed: bool,
        compatibility_fields: &[EngineeringAssemblyCompatibilityField],
        compatibility_issues: &[EngineeringTransitionCompatibilityIssue],
    ) -> String {
        let mut definition = String::new();
        let mut object = Obj::new(&mut definition);
        object.bool("commit_allowed", commit_allowed);
        {
            let mut fields = object.list("compatibility_fields");
            for field in compatibility_fields {
                fields.raw(&field.written());
            }
            fields.end();
        }
        {
            let mut issues = object.list("compatibility_issues");
            for issue in compatibility_issues {
                issues.raw(&issue.written());
            }
            issues.end();
        }
        object.text("current_regime_id", current_regime_id);
        object.raw("guard", &guard.written());
        {
            let mut listed = object.list("identities");
            for identity in identities {
                listed.raw(&identity.written());
            }
            listed.end();
        }
        object.text("operation", operation.name());
        object.text("reconstruction_digest", reconstruction_digest);
        {
            let mut listed = object.list("registers");
            for register in registers {
                listed.raw(&register.written());
            }
            listed.end();
        }
        object.raw("source", &source.written());
        object.raw("target_assembly_draft", &target_assembly_draft.written());
        object.text("target_assembly_hash", target_assembly_hash);
        object.text("target_generator_hash", target_generator_hash);
        object.text("target_regime_id", target_regime_id);
        object.text("target_scenario_hash", target_scenario_hash);
        object.int("version", 3);
        object.end();
        definition
    }

    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let (found, preview_id) = exact_record(value, key, "preview_id")?;
        read::exact_keys(
            found,
            "definition",
            &[
                "commit_allowed",
                "compatibility_fields",
                "compatibility_issues",
                "current_regime_id",
                "guard",
                "identities",
                "operation",
                "reconstruction_digest",
                "registers",
                "source",
                "target_assembly_draft",
                "target_assembly_hash",
                "target_generator_hash",
                "target_regime_id",
                "target_scenario_hash",
                "version",
            ],
        )?;
        if read::int(found, "version", 3, 3)? != 3 {
            return Err(Fault::field("transition_preview"));
        }
        let mut identities = Vec::new();
        for identity in read::list(found, "identities", 128)? {
            identities.push(EngineeringTransitionIdentity::read(identity)?);
        }
        let mut compatibility_issues = Vec::new();
        for issue in read::list(found, "compatibility_issues", 128)? {
            compatibility_issues.push(EngineeringTransitionCompatibilityIssue::read(issue)?);
        }
        let mut compatibility_fields = Vec::new();
        for field in read::list(found, "compatibility_fields", 4096)? {
            compatibility_fields.push(EngineeringAssemblyCompatibilityField::read(field)?);
        }
        let mut registers = Vec::new();
        for register in read::list(found, "registers", 16)? {
            registers.push(EngineeringTransitionRegisterConsequence::read(register)?);
        }
        if !complete_transition_registers(&registers) {
            return Err(Fault::field("registers"));
        }
        let preview = Self::new(
            EngineeringTransitionKind::read(found, "operation")?,
            EngineeringTransitionSource::read(found, "source")?,
            EngineeringTransitionGuard::read(found, "guard")?,
            read::text(found, "current_regime_id")?,
            &crate::state::AssemblyDraft::read(found, "target_assembly_draft")?,
            read::hex(found, "target_generator_hash", 64)?,
            read::hex(found, "target_assembly_hash", 64)?,
            read::text(found, "target_regime_id")?,
            read::hex(found, "target_scenario_hash", 64)?,
            read::hex(found, "reconstruction_digest", 64)?,
            identities,
            registers,
            compatibility_fields,
            compatibility_issues,
        )?;
        if preview.preview_id != preview_id
            || preview.commit_allowed != read::flag(found, "commit_allowed")?
        {
            return Err(Fault::field("preview_id"));
        }
        Ok(preview)
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("definition", &self.definition);
        object.text("preview_id", &self.preview_id);
        object.end();
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineeringTransitionRecoveryState {
    AcceptedUnpersisted,
    PriorRetained,
    ChildPublished,
    PointerMoved,
    Persisted,
    Recovered,
}

impl EngineeringTransitionRecoveryState {
    pub fn name(self) -> &'static str {
        match self {
            EngineeringTransitionRecoveryState::AcceptedUnpersisted => "accepted_unpersisted",
            EngineeringTransitionRecoveryState::PriorRetained => "prior_retained",
            EngineeringTransitionRecoveryState::ChildPublished => "child_published",
            EngineeringTransitionRecoveryState::PointerMoved => "pointer_moved",
            EngineeringTransitionRecoveryState::Persisted => "persisted",
            EngineeringTransitionRecoveryState::Recovered => "recovered",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &[
                "accepted_unpersisted",
                "prior_retained",
                "child_published",
                "pointer_moved",
                "persisted",
                "recovered",
            ],
        )? {
            0 => EngineeringTransitionRecoveryState::AcceptedUnpersisted,
            1 => EngineeringTransitionRecoveryState::PriorRetained,
            2 => EngineeringTransitionRecoveryState::ChildPublished,
            3 => EngineeringTransitionRecoveryState::PointerMoved,
            4 => EngineeringTransitionRecoveryState::Persisted,
            _ => EngineeringTransitionRecoveryState::Recovered,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineeringTransitionClosureReason {
    Restart,
    Superseded,
}

impl EngineeringTransitionClosureReason {
    pub fn name(self) -> &'static str {
        match self {
            EngineeringTransitionClosureReason::Restart => "restart",
            EngineeringTransitionClosureReason::Superseded => "superseded",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(value, key, &["restart", "superseded"])? {
            0 => EngineeringTransitionClosureReason::Restart,
            _ => EngineeringTransitionClosureReason::Superseded,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineeringTransitionReceipt {
    pub after_assembly_hash: String,
    pub after_generator_hash: String,
    pub after_regime_id: String,
    pub after_scenario_hash: String,
    pub before_assembly_hash: String,
    pub before_generator_hash: String,
    pub before_regime_id: String,
    pub before_scenario_hash: String,
    pub child_attempt_id: String,
    pub child_branch_id: String,
    pub closure_reason: EngineeringTransitionClosureReason,
    pub compatibility_fields: Vec<EngineeringAssemblyCompatibilityField>,
    pub compatibility_issues: Vec<EngineeringTransitionCompatibilityIssue>,
    pub detached_evidence_ids: Vec<String>,
    pub identities: Vec<EngineeringTransitionIdentity>,
    pub operation: EngineeringTransitionKind,
    pub operation_id: String,
    pub parent_attempt_id: String,
    pub parent_branch_id: String,
    pub preview_id: String,
    pub reconstruction_digest: String,
    pub registers: Vec<EngineeringTransitionRegisterConsequence>,
    pub recovery_state: EngineeringTransitionRecoveryState,
    pub source: EngineeringTransitionSource,
    pub version: u8,
}

impl EngineeringTransitionReceipt {
    pub fn new(
        preview: &EngineeringTransitionPreview,
        parent_attempt_id: &str,
        parent_branch_id: &str,
        child_attempt_id: &str,
        child_branch_id: &str,
        before_generator_hash: &str,
        before_assembly_hash: &str,
        after_generator_hash: &str,
        after_assembly_hash: &str,
        after_regime_id: &str,
        after_scenario_hash: &str,
        closure_reason: EngineeringTransitionClosureReason,
        reconstruction_digest: &str,
        mut detached_evidence_ids: Vec<String>,
    ) -> Result<Self, Fault> {
        if parent_attempt_id.len() != 16
            || !parent_attempt_id
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || child_attempt_id.len() != 16
            || !child_attempt_id.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !crate::json::is_hex(parent_branch_id, 64)
            || !crate::json::is_hex(child_branch_id, 64)
            || !crate::json::is_hex(before_generator_hash, 64)
            || !crate::json::is_hex(before_assembly_hash, 64)
            || !crate::json::is_hex(after_generator_hash, 64)
            || !crate::json::is_hex(after_assembly_hash, 64)
            || !valid_address(after_regime_id)
            || !crate::json::is_hex(after_scenario_hash, 64)
            || preview.target_generator_hash != after_generator_hash
            || preview.target_assembly_hash != after_assembly_hash
            || preview.target_regime_id != after_regime_id
            || preview.target_scenario_hash != after_scenario_hash
            || !crate::json::is_hex(reconstruction_digest, 64)
            || preview.reconstruction_digest != reconstruction_digest
            || detached_evidence_ids
                .iter()
                .any(|id| !crate::json::is_hex(id, 64))
        {
            return Err(Fault::field("transition_receipt"));
        }
        detached_evidence_ids.sort();
        detached_evidence_ids.dedup();
        let mut receipt = Self {
            after_assembly_hash: after_assembly_hash.to_string(),
            after_generator_hash: after_generator_hash.to_string(),
            after_regime_id: after_regime_id.to_string(),
            after_scenario_hash: after_scenario_hash.to_string(),
            before_assembly_hash: before_assembly_hash.to_string(),
            before_generator_hash: before_generator_hash.to_string(),
            before_regime_id: preview.current_regime_id.clone(),
            before_scenario_hash: preview.guard.scenario_hash.clone(),
            child_attempt_id: child_attempt_id.to_string(),
            child_branch_id: child_branch_id.to_string(),
            closure_reason,
            compatibility_fields: preview.compatibility_fields.clone(),
            compatibility_issues: preview.compatibility_issues.clone(),
            detached_evidence_ids,
            identities: preview.identities.clone(),
            operation: preview.operation,
            operation_id: String::new(),
            parent_attempt_id: parent_attempt_id.to_string(),
            parent_branch_id: parent_branch_id.to_string(),
            preview_id: preview.preview_id.clone(),
            reconstruction_digest: reconstruction_digest.to_string(),
            registers: preview.registers.clone(),
            recovery_state: EngineeringTransitionRecoveryState::AcceptedUnpersisted,
            source: preview.source.clone(),
            version: ENGINEERING_TRANSITION_VERSION,
        };
        receipt.operation_id = definition_id(&receipt.identity_definition());
        Ok(receipt)
    }

    fn write_shape(&self, include_operation_id: bool) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("after_assembly_hash", &self.after_assembly_hash);
        object.text("after_generator_hash", &self.after_generator_hash);
        if self.version >= 5 {
            object.text("after_regime_id", &self.after_regime_id);
            object.text("after_scenario_hash", &self.after_scenario_hash);
        }
        object.text("before_assembly_hash", &self.before_assembly_hash);
        object.text("before_generator_hash", &self.before_generator_hash);
        if self.version >= 5 {
            object.text("before_regime_id", &self.before_regime_id);
            object.text("before_scenario_hash", &self.before_scenario_hash);
        }
        object.text("child_attempt_id", &self.child_attempt_id);
        object.text("child_branch_id", &self.child_branch_id);
        object.text("closure_reason", self.closure_reason.name());
        if self.version >= 4 {
            let mut fields = object.list("compatibility_fields");
            for field in &self.compatibility_fields {
                fields.raw(&field.written());
            }
            fields.end();
        }
        {
            let mut issues = object.list("compatibility_issues");
            for issue in &self.compatibility_issues {
                issues.raw(&issue.written());
            }
            issues.end();
        }
        {
            let mut evidence = object.list("detached_evidence_ids");
            for id in &self.detached_evidence_ids {
                evidence.text(id);
            }
            evidence.end();
        }
        {
            let mut identities = object.list("identities");
            for identity in &self.identities {
                identities.raw(&identity.written());
            }
            identities.end();
        }
        object.text("operation", self.operation.name());
        if include_operation_id {
            object.text("operation_id", &self.operation_id);
        }
        object.text("parent_attempt_id", &self.parent_attempt_id);
        object.text("parent_branch_id", &self.parent_branch_id);
        object.text("preview_id", &self.preview_id);
        object.text("reconstruction_digest", &self.reconstruction_digest);
        {
            let mut registers = object.list("registers");
            for register in &self.registers {
                registers.raw(&register.written());
            }
            registers.end();
        }
        if include_operation_id {
            object.text("recovery_state", self.recovery_state.name());
        }
        object.raw("source", &self.source.written());
        object.int("version", i64::from(self.version));
        object.end();
        out
    }

    fn identity_definition(&self) -> String {
        self.write_shape(false)
    }

    pub fn written(&self) -> String {
        self.write_shape(true)
    }

    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        let version = read::int(
            found,
            "version",
            3,
            i64::from(ENGINEERING_TRANSITION_VERSION),
        )? as u8;
        read::exact_keys(
            found,
            key,
            if version >= 5 {
                &[
                    "after_assembly_hash",
                    "after_generator_hash",
                    "after_regime_id",
                    "after_scenario_hash",
                    "before_assembly_hash",
                    "before_generator_hash",
                    "before_regime_id",
                    "before_scenario_hash",
                    "child_attempt_id",
                    "child_branch_id",
                    "closure_reason",
                    "compatibility_fields",
                    "compatibility_issues",
                    "detached_evidence_ids",
                    "identities",
                    "operation",
                    "operation_id",
                    "parent_attempt_id",
                    "parent_branch_id",
                    "preview_id",
                    "reconstruction_digest",
                    "registers",
                    "recovery_state",
                    "source",
                    "version",
                ]
            } else if version == 4 {
                &[
                    "after_assembly_hash",
                    "after_generator_hash",
                    "before_assembly_hash",
                    "before_generator_hash",
                    "child_attempt_id",
                    "child_branch_id",
                    "closure_reason",
                    "compatibility_fields",
                    "compatibility_issues",
                    "detached_evidence_ids",
                    "identities",
                    "operation",
                    "operation_id",
                    "parent_attempt_id",
                    "parent_branch_id",
                    "preview_id",
                    "reconstruction_digest",
                    "registers",
                    "recovery_state",
                    "source",
                    "version",
                ]
            } else {
                &[
                "after_assembly_hash",
                "after_generator_hash",
                "before_assembly_hash",
                "before_generator_hash",
                "child_attempt_id",
                "child_branch_id",
                "closure_reason",
                "compatibility_issues",
                "detached_evidence_ids",
                "identities",
                "operation",
                "operation_id",
                "parent_attempt_id",
                "parent_branch_id",
                "preview_id",
                "reconstruction_digest",
                "registers",
                "recovery_state",
                "source",
                "version",
                ]
            },
        )?;
        let mut identities = Vec::new();
        for identity in read::list(found, "identities", 128)? {
            identities.push(EngineeringTransitionIdentity::read(identity)?);
        }
        let mut compatibility_issues = Vec::new();
        for issue in read::list(found, "compatibility_issues", 128)? {
            compatibility_issues.push(EngineeringTransitionCompatibilityIssue::read(issue)?);
        }
        let mut compatibility_fields = Vec::new();
        if version >= 4 {
            for field in read::list(found, "compatibility_fields", 4096)? {
                compatibility_fields.push(EngineeringAssemblyCompatibilityField::read(field)?);
            }
        }
        let mut detached_evidence_ids = Vec::new();
        for id in read::list(found, "detached_evidence_ids", 256)? {
            match id {
                Json::Text(id) if crate::json::is_hex(id, 64) => {
                    detached_evidence_ids.push(id.clone())
                }
                _ => return Err(Fault::field("detached_evidence_ids")),
            }
        }
        if !read::ascending(&detached_evidence_ids) {
            return Err(Fault::field("detached_evidence_ids"));
        }
        let mut registers = Vec::new();
        for register in read::list(found, "registers", 16)? {
            registers.push(EngineeringTransitionRegisterConsequence::read(register)?);
        }
        if !complete_transition_registers(&registers) {
            return Err(Fault::field("registers"));
        }
        let receipt = Self {
            after_assembly_hash: read::hex(found, "after_assembly_hash", 64)?.to_string(),
            after_generator_hash: read::hex(found, "after_generator_hash", 64)?.to_string(),
            after_regime_id: if version >= 5 {
                read::text(found, "after_regime_id")?.to_string()
            } else {
                String::new()
            },
            after_scenario_hash: if version >= 5 {
                read::hex(found, "after_scenario_hash", 64)?.to_string()
            } else {
                String::new()
            },
            before_assembly_hash: read::hex(found, "before_assembly_hash", 64)?.to_string(),
            before_generator_hash: read::hex(found, "before_generator_hash", 64)?.to_string(),
            before_regime_id: if version >= 5 {
                read::text(found, "before_regime_id")?.to_string()
            } else {
                String::new()
            },
            before_scenario_hash: if version >= 5 {
                read::hex(found, "before_scenario_hash", 64)?.to_string()
            } else {
                String::new()
            },
            child_attempt_id: read_attempt_id(found, "child_attempt_id")?,
            child_branch_id: read::hex(found, "child_branch_id", 64)?.to_string(),
            closure_reason: EngineeringTransitionClosureReason::read(
                found,
                "closure_reason",
            )?,
            compatibility_fields,
            compatibility_issues,
            detached_evidence_ids,
            identities,
            operation: EngineeringTransitionKind::read(found, "operation")?,
            operation_id: read::hex(found, "operation_id", 64)?.to_string(),
            parent_attempt_id: read_attempt_id(found, "parent_attempt_id")?,
            parent_branch_id: read::hex(found, "parent_branch_id", 64)?.to_string(),
            preview_id: read::hex(found, "preview_id", 64)?.to_string(),
            reconstruction_digest: read::hex(found, "reconstruction_digest", 64)?.to_string(),
            registers,
            recovery_state: EngineeringTransitionRecoveryState::read(found, "recovery_state")?,
            source: EngineeringTransitionSource::read(found, "source")?,
            version,
        };
        if (version >= 5
            && (!valid_address(&receipt.after_regime_id)
                || !valid_address(&receipt.before_regime_id)))
            || !receipt.source.supports(receipt.operation)
            || definition_id(&receipt.identity_definition()) != receipt.operation_id
        {
            return Err(Fault::field(key));
        }
        Ok(receipt)
    }

    pub fn coherent_child(
        &self,
        attempt_id: &str,
        branch_id: &str,
        generator_hash: &str,
        assembly_hash: &str,
    ) -> bool {
        self.child_attempt_id == attempt_id
            && self.child_branch_id == branch_id
            && self.after_generator_hash == generator_hash
            && self.after_assembly_hash == assembly_hash
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceAuthority {
    CommittedDesign,
    QualificationResult,
    MigratedV1,
}

impl SourceAuthority {
    pub fn name(self) -> &'static str {
        match self {
            SourceAuthority::CommittedDesign => "committed_design",
            SourceAuthority::QualificationResult => "qualification_result",
            SourceAuthority::MigratedV1 => "migrated_v1",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &["committed_design", "qualification_result", "migrated_v1"],
        )? {
            0 => SourceAuthority::CommittedDesign,
            1 => SourceAuthority::QualificationResult,
            _ => SourceAuthority::MigratedV1,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordSource {
    pub authority: SourceAuthority,
    pub attempt_id: String,
    pub branch_id: String,
    pub result_id: Option<String>,
}

impl RecordSource {
    pub fn committed(attempt_id: &str, branch_id: &str) -> Self {
        Self {
            authority: SourceAuthority::CommittedDesign,
            attempt_id: attempt_id.to_string(),
            branch_id: branch_id.to_string(),
            result_id: None,
        }
    }

    pub fn result(attempt_id: &str, branch_id: &str, result_id: &str) -> Self {
        Self {
            authority: SourceAuthority::QualificationResult,
            attempt_id: attempt_id.to_string(),
            branch_id: branch_id.to_string(),
            result_id: Some(result_id.to_string()),
        }
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("attempt_id", &self.attempt_id);
        object.text("authority", self.authority.name());
        object.text("branch_id", &self.branch_id);
        match &self.result_id {
            Some(id) => object.text("result_id", id),
            None => object.null("result_id"),
        };
        object.int("version", 1);
        object.end();
        out
    }

    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let found = read::map(value, key)?;
        read::exact_keys(
            found,
            key,
            &["attempt_id", "authority", "branch_id", "result_id", "version"],
        )?;
        if read::int(found, "version", 1, 1)? != 1 {
            return Err(Fault::field(key));
        }
        let authority = SourceAuthority::read(found, "authority")?;
        let result_id = read_nullable_hex(found, "result_id")?;
        if matches!(authority, SourceAuthority::QualificationResult) != result_id.is_some() {
            return Err(Fault::field("result_id"));
        }
        Ok(Self {
            authority,
            attempt_id: read_attempt_id(found, "attempt_id")?,
            branch_id: read::hex(found, "branch_id", 64)?.to_string(),
            result_id,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DerivationSourceKind {
    AttemptBranch,
    Blueprint,
    QualificationRequest,
    QualificationResult,
    LegacyBlueprint,
}

impl DerivationSourceKind {
    pub fn name(self) -> &'static str {
        match self {
            DerivationSourceKind::AttemptBranch => "attempt_branch",
            DerivationSourceKind::Blueprint => "blueprint",
            DerivationSourceKind::QualificationRequest => "qualification_request",
            DerivationSourceKind::QualificationResult => "qualification_result",
            DerivationSourceKind::LegacyBlueprint => "legacy_blueprint",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &[
                "attempt_branch",
                "blueprint",
                "qualification_request",
                "qualification_result",
                "legacy_blueprint",
            ],
        )? {
            0 => DerivationSourceKind::AttemptBranch,
            1 => DerivationSourceKind::Blueprint,
            2 => DerivationSourceKind::QualificationRequest,
            3 => DerivationSourceKind::QualificationResult,
            _ => DerivationSourceKind::LegacyBlueprint,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DerivationOperation {
    Capture,
    Promote,
    Clone,
    HypothesisBranch,
    Transplant,
    AssemblyAdaptation,
}

impl DerivationOperation {
    pub fn name(self) -> &'static str {
        match self {
            DerivationOperation::Capture => "capture",
            DerivationOperation::Promote => "promote",
            DerivationOperation::Clone => "clone",
            DerivationOperation::HypothesisBranch => "hypothesis_branch",
            DerivationOperation::Transplant => "transplant",
            DerivationOperation::AssemblyAdaptation => "assembly_adaptation",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &[
                "capture",
                "promote",
                "clone",
                "hypothesis_branch",
                "transplant",
                "assembly_adaptation",
            ],
        )? {
            0 => DerivationOperation::Capture,
            1 => DerivationOperation::Promote,
            2 => DerivationOperation::Clone,
            3 => DerivationOperation::HypothesisBranch,
            4 => DerivationOperation::Transplant,
            _ => DerivationOperation::AssemblyAdaptation,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivationEdge {
    pub operation: DerivationOperation,
    pub source_id: String,
    pub source_kind: DerivationSourceKind,
}

impl DerivationEdge {
    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("operation", self.operation.name());
        object.text("source_id", &self.source_id);
        object.text("source_kind", self.source_kind.name());
        object.int("version", 1);
        object.end();
        out
    }

    pub fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(
            value,
            "derivation_edges",
            &["operation", "source_id", "source_kind", "version"],
        )?;
        if read::int(value, "version", 1, 1)? != 1 {
            return Err(Fault::field("derivation_edges"));
        }
        Ok(Self {
            operation: DerivationOperation::read(value, "operation")?,
            source_id: read::hex(value, "source_id", 64)?.to_string(),
            source_kind: DerivationSourceKind::read(value, "source_kind")?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceKind {
    QualificationRequest,
    QualificationResult,
    FailureTrace,
    ComparativeQualification,
}

impl EvidenceKind {
    pub fn name(self) -> &'static str {
        match self {
            EvidenceKind::QualificationRequest => "qualification_request",
            EvidenceKind::QualificationResult => "qualification_result",
            EvidenceKind::FailureTrace => "failure_trace",
            EvidenceKind::ComparativeQualification => "comparative_qualification",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &[
                "qualification_request",
                "qualification_result",
                "failure_trace",
                "comparative_qualification",
            ],
        )? {
            0 => EvidenceKind::QualificationRequest,
            1 => EvidenceKind::QualificationResult,
            2 => EvidenceKind::FailureTrace,
            _ => EvidenceKind::ComparativeQualification,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceRole {
    SourceQualification,
    Diagnostic,
    ComparisonSource,
    FirstFailure,
}

impl EvidenceRole {
    pub fn name(self) -> &'static str {
        match self {
            EvidenceRole::SourceQualification => "source_qualification",
            EvidenceRole::Diagnostic => "diagnostic",
            EvidenceRole::ComparisonSource => "comparison_source",
            EvidenceRole::FirstFailure => "first_failure",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &["source_qualification", "diagnostic", "comparison_source", "first_failure"],
        )? {
            0 => EvidenceRole::SourceQualification,
            1 => EvidenceRole::Diagnostic,
            2 => EvidenceRole::ComparisonSource,
            _ => EvidenceRole::FirstFailure,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceAvailability {
    Available,
    Unavailable,
}

impl EvidenceAvailability {
    pub fn name(self) -> &'static str {
        match self {
            EvidenceAvailability::Available => "available",
            EvidenceAvailability::Unavailable => "unavailable",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(value, key, &["available", "unavailable"])? {
            0 => EvidenceAvailability::Available,
            _ => EvidenceAvailability::Unavailable,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceLink {
    pub availability: EvidenceAvailability,
    pub evidence_id: String,
    pub evidence_kind: EvidenceKind,
    pub role: EvidenceRole,
}

impl EvidenceLink {
    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("availability", self.availability.name());
        object.text("evidence_id", &self.evidence_id);
        object.text("evidence_kind", self.evidence_kind.name());
        object.text("role", self.role.name());
        object.int("version", 1);
        object.end();
        out
    }

    pub fn read(value: &Json) -> Result<Self, Fault> {
        read::exact_keys(
            value,
            "evidence_links",
            &["availability", "evidence_id", "evidence_kind", "role", "version"],
        )?;
        if read::int(value, "version", 1, 1)? != 1 {
            return Err(Fault::field("evidence_links"));
        }
        Ok(Self {
            availability: EvidenceAvailability::read(value, "availability")?,
            evidence_id: read::hex(value, "evidence_id", 64)?.to_string(),
            evidence_kind: EvidenceKind::read(value, "evidence_kind")?,
            role: EvidenceRole::read(value, "role")?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlueprintCreationReason {
    DesignCapture,
    ResultCapture,
    V1Promotion,
    Clone,
    Transplant,
}

impl BlueprintCreationReason {
    pub fn name(self) -> &'static str {
        match self {
            BlueprintCreationReason::DesignCapture => "design_capture",
            BlueprintCreationReason::ResultCapture => "result_capture",
            BlueprintCreationReason::V1Promotion => "v1_promotion",
            BlueprintCreationReason::Clone => "clone",
            BlueprintCreationReason::Transplant => "transplant",
        }
    }

    fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        Ok(match read::one_of(
            value,
            key,
            &["design_capture", "result_capture", "v1_promotion", "clone", "transplant"],
        )? {
            0 => BlueprintCreationReason::DesignCapture,
            1 => BlueprintCreationReason::ResultCapture,
            2 => BlueprintCreationReason::V1Promotion,
            3 => BlueprintCreationReason::Clone,
            _ => BlueprintCreationReason::Transplant,
        })
    }
}

#[derive(Clone, Debug)]
pub struct GeneratorRecordV2 {
    pub content_hash: String,
    pub contract_id: String,
    pub definition: String,
    pub generator_spec: GeneratorSpec,
    pub generator_record_id: String,
    pub protocol_version: u32,
    pub source: RecordSource,
}

impl GeneratorRecordV2 {
    pub fn new(
        generator: &GeneratorSpec,
        contract_id: &str,
        content_hash: &str,
        source: &RecordSource,
    ) -> Self {
        let mut definition = String::new();
        let mut object = Obj::new(&mut definition);
        object.text("content_hash", content_hash);
        object.text("contract_id", contract_id);
        object.raw("generator_spec", &generator.written());
        object.text("generator_spec_hash", &generator.specification_hash());
        object.int("protocol_version", i64::from(crate::protocol::PROTOCOL_VERSION));
        object.raw("source", &source.written());
        object.int("version", i64::from(ENGINEERING_RECORD_VERSION));
        object.end();
        let generator_record_id = definition_id(&definition);
        Self {
            content_hash: content_hash.to_string(),
            contract_id: contract_id.to_string(),
            definition,
            generator_spec: generator.clone(),
            generator_record_id,
            protocol_version: crate::protocol::PROTOCOL_VERSION,
            source: source.clone(),
        }
    }

    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let (found, record_id) = exact_record(value, key, "generator_record_id")?;
        read::exact_keys(
            found,
            "definition",
            &[
                "content_hash",
                "contract_id",
                "generator_spec",
                "generator_spec_hash",
                "protocol_version",
                "source",
                "version",
            ],
        )?;
        if read::int(found, "version", 2, 2)? != 2 {
            return Err(Fault::field("version"));
        }
        let contract_id = read::text(found, "contract_id")?;
        if !valid_contract_id(contract_id) {
            return Err(Fault::field("contract_id"));
        }
        let generator_spec = GeneratorSpec::read(found, "generator_spec")?;
        if read::hex(found, "generator_spec_hash", 64)? != generator_spec.specification_hash() {
            return Err(Fault::field("generator_spec_hash"));
        }
        let protocol_version = read::int(
            found,
            "protocol_version",
            1,
            i64::from(crate::protocol::PROTOCOL_VERSION),
        )? as u32;
        let definition = canonical_definition(found)?;
        if definition_id(&definition) != record_id {
            return Err(Fault::field("generator_record_id"));
        }
        Ok(Self {
            content_hash: read::hex(found, "content_hash", 64)?.to_string(),
            contract_id: contract_id.to_string(),
            definition,
            generator_spec,
            generator_record_id: record_id.to_string(),
            protocol_version,
            source: RecordSource::read(found, "source")?,
        })
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("definition", &self.definition);
        object.text("generator_record_id", &self.generator_record_id);
        object.end();
        out
    }
}

#[derive(Clone, Debug)]
pub struct AssemblyRecordV2 {
    pub assembly_template: AssemblyTemplate,
    pub assembly_record_id: String,
    pub content_hash: String,
    pub contract_id: String,
    pub definition: String,
    pub generator_record_id: String,
    pub protocol_version: u32,
    pub regime_id: String,
    pub source: RecordSource,
}

impl AssemblyRecordV2 {
    pub fn new(
        assembly: &AssemblyTemplate,
        contract_id: &str,
        content_hash: &str,
        generator_record_id: &str,
        regime_id: &str,
        source: &RecordSource,
    ) -> Self {
        let mut definition = String::new();
        let mut object = Obj::new(&mut definition);
        object.raw("assembly_template", &assembly.written());
        object.text("assembly_template_hash", assembly.hash());
        {
            let mut compatibility = object.object("compatibility");
            compatibility.text("content_hash", content_hash);
            compatibility.text("contract_id", contract_id);
            compatibility.text("generator_record_id", generator_record_id);
            compatibility.text("regime", regime_id);
            compatibility.text("run_kind", "automation_contract");
            compatibility.int("version", i64::from(ENGINEERING_RECORD_VERSION));
            compatibility.end();
        }
        {
            let mut fields = object.list("owned_fields");
            for field in ASSEMBLY_OWNED_FIELDS {
                fields.text(field);
            }
            fields.end();
        }
        object.int("protocol_version", i64::from(crate::protocol::PROTOCOL_VERSION));
        object.raw("source", &source.written());
        object.int("version", i64::from(ENGINEERING_RECORD_VERSION));
        object.end();
        let assembly_record_id = definition_id(&definition);
        Self {
            assembly_template: assembly.clone(),
            assembly_record_id,
            content_hash: content_hash.to_string(),
            contract_id: contract_id.to_string(),
            definition,
            generator_record_id: generator_record_id.to_string(),
            protocol_version: crate::protocol::PROTOCOL_VERSION,
            regime_id: regime_id.to_string(),
            source: source.clone(),
        }
    }

    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let (found, record_id) = exact_record(value, key, "assembly_record_id")?;
        read::exact_keys(
            found,
            "definition",
            &[
                "assembly_template",
                "assembly_template_hash",
                "compatibility",
                "owned_fields",
                "protocol_version",
                "source",
                "version",
            ],
        )?;
        if read::int(found, "version", 2, 2)? != 2 {
            return Err(Fault::field("version"));
        }
        let assembly_template = AssemblyTemplate::read(found, "assembly_template")?
            .filter(|template| template.is_exact() && template.version() == 2)
            .ok_or_else(|| Fault::field("assembly_template"))?;
        if read::hex(found, "assembly_template_hash", 64)? != assembly_template.hash() {
            return Err(Fault::field("assembly_template_hash"));
        }
        let compatibility = read::map(found, "compatibility")?;
        read::exact_keys(
            compatibility,
            "compatibility",
            &[
                "content_hash",
                "contract_id",
                "generator_record_id",
                "regime",
                "run_kind",
                "version",
            ],
        )?;
        if read::int(compatibility, "version", 2, 2)? != 2
            || read::text(compatibility, "run_kind")? != "automation_contract"
        {
            return Err(Fault::field("compatibility"));
        }
        let contract_id = read::text(compatibility, "contract_id")?;
        if !valid_contract_id(contract_id) {
            return Err(Fault::field("contract_id"));
        }
        read::one_of(compatibility, "regime", &REGIME_IDS)?;
        let owned_fields = read::list(found, "owned_fields", ASSEMBLY_OWNED_FIELDS.len())?;
        if owned_fields.len() != ASSEMBLY_OWNED_FIELDS.len()
            || owned_fields.iter().zip(ASSEMBLY_OWNED_FIELDS).any(|(held, expected)| {
                !matches!(held, Json::Text(value) if value == expected)
            })
        {
            return Err(Fault::field("owned_fields"));
        }
        let protocol_version = read::int(
            found,
            "protocol_version",
            1,
            i64::from(crate::protocol::PROTOCOL_VERSION),
        )? as u32;
        let definition = canonical_definition(found)?;
        if definition_id(&definition) != record_id {
            return Err(Fault::field("assembly_record_id"));
        }
        Ok(Self {
            assembly_template,
            assembly_record_id: record_id.to_string(),
            content_hash: read::hex(compatibility, "content_hash", 64)?.to_string(),
            contract_id: contract_id.to_string(),
            definition,
            generator_record_id: read::hex(compatibility, "generator_record_id", 64)?.to_string(),
            protocol_version,
            regime_id: read::text(compatibility, "regime")?.to_string(),
            source: RecordSource::read(found, "source")?,
        })
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("assembly_record_id", &self.assembly_record_id);
        object.raw("definition", &self.definition);
        object.end();
        out
    }
}

#[derive(Clone, Debug)]
pub struct BlueprintRecordV2 {
    pub assembly_record_id: String,
    pub blueprint_id: String,
    pub content_hash: String,
    pub contract_id: String,
    pub creation_reason: BlueprintCreationReason,
    pub definition: String,
    pub derivation_edges: Vec<DerivationEdge>,
    pub evidence_links: Vec<EvidenceLink>,
    pub generator_record_id: String,
    pub parent_blueprint_id: Option<String>,
    pub protocol_version: u32,
    pub source_attempt_id: String,
    pub source_branch_id: String,
}

pub struct BlueprintRecordInput<'a> {
    pub assembly_record_id: &'a str,
    pub contract_id: &'a str,
    pub content_hash: &'a str,
    pub creation_reason: BlueprintCreationReason,
    pub derivation_edges: &'a [DerivationEdge],
    pub evidence_links: &'a [EvidenceLink],
    pub generator_record_id: &'a str,
    pub parent_blueprint_id: Option<&'a str>,
    pub source: &'a RecordSource,
}

impl BlueprintRecordV2 {
    pub fn new(input: BlueprintRecordInput<'_>) -> Self {
        let mut definition = String::new();
        let mut object = Obj::new(&mut definition);
        object.text("assembly_record_id", input.assembly_record_id);
        object.text("content_hash", input.content_hash);
        object.text("contract_id", input.contract_id);
        object.text("creation_reason", input.creation_reason.name());
        {
            let mut edges = object.list("derivation_edges");
            for edge in input.derivation_edges {
                edges.raw(&edge.written());
            }
            edges.end();
        }
        {
            let mut links = object.list("evidence_links");
            for link in input.evidence_links {
                links.raw(&link.written());
            }
            links.end();
        }
        object.text("generator_record_id", input.generator_record_id);
        match input.parent_blueprint_id {
            Some(id) => object.text("parent_blueprint_id", id),
            None => object.null("parent_blueprint_id"),
        };
        object.int("protocol_version", i64::from(crate::protocol::PROTOCOL_VERSION));
        object.text("source_attempt_id", &input.source.attempt_id);
        object.text("source_branch_id", &input.source.branch_id);
        object.int("version", i64::from(ENGINEERING_RECORD_VERSION));
        object.end();
        let blueprint_id = definition_id(&definition);
        Self {
            assembly_record_id: input.assembly_record_id.to_string(),
            blueprint_id,
            content_hash: input.content_hash.to_string(),
            contract_id: input.contract_id.to_string(),
            creation_reason: input.creation_reason,
            definition,
            derivation_edges: input.derivation_edges.to_vec(),
            evidence_links: input.evidence_links.to_vec(),
            generator_record_id: input.generator_record_id.to_string(),
            parent_blueprint_id: input.parent_blueprint_id.map(str::to_string),
            protocol_version: crate::protocol::PROTOCOL_VERSION,
            source_attempt_id: input.source.attempt_id.clone(),
            source_branch_id: input.source.branch_id.clone(),
        }
    }

    pub fn read(value: &Json, key: &str) -> Result<Self, Fault> {
        let (found, record_id) = exact_record(value, key, "blueprint_id")?;
        read::exact_keys(
            found,
            "definition",
            &[
                "assembly_record_id",
                "content_hash",
                "contract_id",
                "creation_reason",
                "derivation_edges",
                "evidence_links",
                "generator_record_id",
                "parent_blueprint_id",
                "protocol_version",
                "source_attempt_id",
                "source_branch_id",
                "version",
            ],
        )?;
        if read::int(found, "version", 2, 2)? != 2 {
            return Err(Fault::field("version"));
        }
        let contract_id = read::text(found, "contract_id")?;
        if !valid_contract_id(contract_id) {
            return Err(Fault::field("contract_id"));
        }
        let mut derivation_edges = Vec::new();
        for edge in read::list(found, "derivation_edges", 64)? {
            derivation_edges.push(DerivationEdge::read(edge)?);
        }
        let mut evidence_links = Vec::new();
        for link in read::list(found, "evidence_links", 64)? {
            evidence_links.push(EvidenceLink::read(link)?);
        }
        let protocol_version = read::int(
            found,
            "protocol_version",
            1,
            i64::from(crate::protocol::PROTOCOL_VERSION),
        )? as u32;
        let definition = canonical_definition(found)?;
        if definition_id(&definition) != record_id {
            return Err(Fault::field("blueprint_id"));
        }
        Ok(Self {
            assembly_record_id: read::hex(found, "assembly_record_id", 64)?.to_string(),
            blueprint_id: record_id.to_string(),
            content_hash: read::hex(found, "content_hash", 64)?.to_string(),
            contract_id: contract_id.to_string(),
            creation_reason: BlueprintCreationReason::read(found, "creation_reason")?,
            definition,
            derivation_edges,
            evidence_links,
            generator_record_id: read::hex(found, "generator_record_id", 64)?.to_string(),
            parent_blueprint_id: read_nullable_hex(found, "parent_blueprint_id")?,
            protocol_version,
            source_attempt_id: read_attempt_id(found, "source_attempt_id")?,
            source_branch_id: read::hex(found, "source_branch_id", 64)?.to_string(),
        })
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.text("blueprint_id", &self.blueprint_id);
        object.raw("definition", &self.definition);
        object.end();
        out
    }
}

fn component_draft_written(component: &AssemblyComponentDraft) -> String {
    let mut out = String::new();
    let mut object = Obj::new(&mut out);
    object.int("layer", i64::from(component.layer));
    object.int("node", i64::from(component.node));
    object.bool("open", component.open);
    object.raw("pos", &component.pos.written());
    object.int("q", component.q);
    object.end();
    out
}

fn current_draft_written(current: &AssemblyCurrentDraft) -> String {
    let mut out = String::new();
    let mut object = Obj::new(&mut out);
    object.bool("active", current.active);
    object.int("current", i64::from(current.current));
    object.int("phase", i64::from(current.phase));
    object.end();
    out
}

fn form_draft_written(form: &AssemblyFormDraft) -> String {
    let mut out = String::new();
    let mut object = Obj::new(&mut out);
    object.int_or_null("junction_blanks", form.junction_blanks.map(i64::from));
    object.int("node", i64::from(form.node));
    object.int("reserve", form.reserve);
    object.end();
    out
}

fn material_draft_written(material: &AssemblyMaterialDraft) -> String {
    let mut out = String::new();
    let mut object = Obj::new(&mut out);
    object.int("amount", i64::from(material.amount));
    object.int("layer", i64::from(material.layer));
    object.int("material", i64::from(material.material));
    object.raw("pos", &material.pos.written());
    object.end();
    out
}

fn compartment_written(compartment: &crate::field::PhysicalCompartment) -> String {
    let mut out = String::new();
    compartment.write(&mut out);
    out
}

fn write_diff_entry(
    entries: &mut crate::json::Arr<'_>,
    address: &str,
    after: &str,
    before: &str,
    kind: &str,
) {
    let mut entry = entries.object();
    entry.text("address", address);
    entry.raw("after", after);
    entry.raw("before", before);
    entry.text("kind", kind);
    entry.int("version", 1);
    entry.end();
}

#[derive(Clone, Debug)]
pub struct AssemblyDiffV1 {
    pub definition: String,
    pub diff_id: String,
}

impl AssemblyDiffV1 {
    pub fn between(
        before_hash: &str,
        before: &AssemblyDraft,
        after_hash: &str,
        after: &AssemblyDraft,
    ) -> Self {
        let mut definition = String::new();
        let mut object = Obj::new(&mut definition);
        object.text("after_assembly_hash", after_hash);
        object.text("before_assembly_hash", before_hash);
        {
            let mut changes = object.list("changes");
            for (left, right) in before.components.iter().zip(&after.components) {
                if left != right {
                    write_diff_entry(
                        &mut changes,
                        &format!("component:{}", left.node),
                        &component_draft_written(right),
                        &component_draft_written(left),
                        "component",
                    );
                }
            }
            for (left, right) in before.currents.iter().zip(&after.currents) {
                if left != right {
                    write_diff_entry(
                        &mut changes,
                        &format!("current:{}", left.current),
                        &current_draft_written(right),
                        &current_draft_written(left),
                        "current",
                    );
                }
            }
            for (left, right) in before.forms.iter().zip(&after.forms) {
                if left != right {
                    write_diff_entry(
                        &mut changes,
                        &format!("form:{}", left.node),
                        &form_draft_written(right),
                        &form_draft_written(left),
                        "form",
                    );
                }
            }
            for (left, right) in before.materials.iter().zip(&after.materials) {
                if left != right {
                    write_diff_entry(
                        &mut changes,
                        &format!("material:{}", left.material),
                        &material_draft_written(right),
                        &material_draft_written(left),
                        "material",
                    );
                }
            }
            if before.physical_compartment != after.physical_compartment {
                write_diff_entry(
                    &mut changes,
                    "physical_compartment",
                    &compartment_written(&after.physical_compartment),
                    &compartment_written(&before.physical_compartment),
                    "physical_compartment",
                );
            }
            changes.end();
        }
        object.int("version", 1);
        object.end();
        let diff_id = definition_id(&definition);
        Self { definition, diff_id }
    }

    pub fn written(&self) -> String {
        let mut out = String::new();
        let mut object = Obj::new(&mut out);
        object.raw("definition", &self.definition);
        object.text("diff_id", &self.diff_id);
        object.end();
        out
    }
}
