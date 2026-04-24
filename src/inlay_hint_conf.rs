use std::io;

use ra_ap_hir_ty::display::ClosureStyle;
use ra_ap_ide::{
    AdjustmentHints, AdjustmentHintsMode,
    ClosureReturnTypeHints, DiscriminantHints,
    GenericParameterHints, InlayFieldsToResolve,
    InlayHintsConfig, LifetimeElisionHints,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InlayHintsConfigDe {
    pub adjustment_hints: AdjustmentHintsDe,
    pub adjustment_hints_disable_reborrows: bool,
    pub adjustment_hints_hide_outside_unsafe: bool,
    pub adjustment_hints_mode: AdjustmentHintsModeDe,
    pub binding_mode_hints: bool,
    pub chaining_hints: bool,
    pub closing_brace_hints_min_lines: Option<usize>,
    pub closure_capture_hints: bool,
    pub closure_return_type_hints: ClosureReturnTypeHintsDe,
    pub closure_style: ClosureStyleDe,
    pub discriminant_hints: DiscriminantHintsDe,
    pub fields_to_resolve: InlayFieldsToResolveDe,
    pub generic_parameter_hints: GenericParameterHintsDe,
    pub hide_closure_initialization_hints: bool,
    pub hide_closure_parameter_hints: bool,
    pub hide_inferred_type_hints: bool,
    pub hide_named_constructor_hints: bool,
    pub implicit_drop_hints: bool,
    pub implied_dyn_trait_hints: bool,
    pub lifetime_elision_hints: LifetimeElisionHintsDe,
    pub max_length: Option<usize>,
    pub param_names_for_lifetime_elision_hints: bool,
    pub parameter_hints: bool,
    pub parameter_hints_for_missing_arguments: bool,
    pub range_exclusive_hints: bool,
    pub render_colons: bool,
    pub sized_bound: bool,
    pub type_hints: bool,
}

impl<'a> From<InlayHintsConfigDe> for InlayHintsConfig<'a> {
    fn from(c: InlayHintsConfigDe) -> Self {
        InlayHintsConfig {
            adjustment_hints: c.adjustment_hints.into(),
            adjustment_hints_disable_reborrows: c
                .adjustment_hints_disable_reborrows,
            adjustment_hints_hide_outside_unsafe: c
                .adjustment_hints_hide_outside_unsafe,
            adjustment_hints_mode: c
                .adjustment_hints_mode
                .into(),
            binding_mode_hints: c.binding_mode_hints,
            chaining_hints: c.chaining_hints,
            closing_brace_hints_min_lines: c
                .closing_brace_hints_min_lines,
            closure_capture_hints: c.closure_capture_hints,
            closure_return_type_hints: c
                .closure_return_type_hints
                .into(),
            closure_style: c.closure_style.into(),
            discriminant_hints: c.discriminant_hints.into(),
            fields_to_resolve: c.fields_to_resolve.into(),
            generic_parameter_hints: c
                .generic_parameter_hints
                .into(),
            hide_closure_initialization_hints: c
                .hide_closure_initialization_hints,
            hide_closure_parameter_hints: c
                .hide_closure_parameter_hints,
            hide_inferred_type_hints: c.hide_inferred_type_hints,
            hide_named_constructor_hints: c
                .hide_named_constructor_hints,
            implicit_drop_hints: c.implicit_drop_hints,
            implied_dyn_trait_hints: c.implied_dyn_trait_hints,
            lifetime_elision_hints: c
                .lifetime_elision_hints
                .into(),
            max_length: c.max_length,
            minicore: Default::default(),
            param_names_for_lifetime_elision_hints: c
                .param_names_for_lifetime_elision_hints,
            parameter_hints: c.parameter_hints,
            parameter_hints_for_missing_arguments: c
                .parameter_hints_for_missing_arguments,
            range_exclusive_hints: c.range_exclusive_hints,
            render_colons: c.render_colons,
            sized_bound: c.sized_bound,
            type_hints: c.type_hints,
        }
    }
}

#[derive(Deserialize)]
pub enum AdjustmentHintsDe {
    Always,
    ReBorrowOnly,
    Never,
}

impl From<AdjustmentHintsDe> for AdjustmentHints {
    fn from(v: AdjustmentHintsDe) -> Self {
        match v {
            AdjustmentHintsDe::Always => AdjustmentHints::Always,
            AdjustmentHintsDe::ReBorrowOnly => {
                AdjustmentHints::BorrowsOnly
            }
            AdjustmentHintsDe::Never => AdjustmentHints::Never,
        }
    }
}

#[derive(Deserialize)]
pub enum AdjustmentHintsModeDe {
    Prefix,
    Postfix,
    PreferPrefix,
    PreferPostfix,
}

impl From<AdjustmentHintsModeDe> for AdjustmentHintsMode {
    fn from(v: AdjustmentHintsModeDe) -> Self {
        match v {
            AdjustmentHintsModeDe::Prefix => {
                AdjustmentHintsMode::Prefix
            }
            AdjustmentHintsModeDe::Postfix => {
                AdjustmentHintsMode::Postfix
            }
            AdjustmentHintsModeDe::PreferPrefix => {
                AdjustmentHintsMode::PreferPrefix
            }
            AdjustmentHintsModeDe::PreferPostfix => {
                AdjustmentHintsMode::PreferPostfix
            }
        }
    }
}

#[derive(Deserialize)]
pub enum ClosureReturnTypeHintsDe {
    Always,
    Never,
    WithBlock,
}

impl From<ClosureReturnTypeHintsDe> for ClosureReturnTypeHints {
    fn from(v: ClosureReturnTypeHintsDe) -> Self {
        match v {
            ClosureReturnTypeHintsDe::Always => {
                ClosureReturnTypeHints::Always
            }
            ClosureReturnTypeHintsDe::Never => {
                ClosureReturnTypeHints::Never
            }
            ClosureReturnTypeHintsDe::WithBlock => {
                ClosureReturnTypeHints::WithBlock
            }
        }
    }
}

pub enum ClosureStyleDe {
    ImplFn,
    RustAnalyzer,
    WithId,
    Hide,
}

impl<'de> Deserialize<'de> for ClosureStyleDe {
    fn deserialize<D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.as_str() {
            "ImplFn" => Ok(ClosureStyleDe::ImplFn),
            "RANotation" => Ok(ClosureStyleDe::RustAnalyzer),
            "WithId" => Ok(ClosureStyleDe::WithId),
            "Hide" => Ok(ClosureStyleDe::Hide),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["ImplFn", "RANotation", "WithId", "Hide"],
            )),
        }
    }
}

impl From<ClosureStyleDe> for ClosureStyle {
    fn from(v: ClosureStyleDe) -> Self {
        match v {
            ClosureStyleDe::ImplFn => ClosureStyle::ImplFn,
            ClosureStyleDe::RustAnalyzer => {
                ClosureStyle::RANotation
            }
            ClosureStyleDe::WithId => {
                ClosureStyle::ClosureWithId
            }
            ClosureStyleDe::Hide => ClosureStyle::Hide,
        }
    }
}

#[derive(Deserialize)]
pub enum DiscriminantHintsDe {
    Always,
    Never,
    Fieldless,
}

impl From<DiscriminantHintsDe> for DiscriminantHints {
    fn from(v: DiscriminantHintsDe) -> Self {
        match v {
            DiscriminantHintsDe::Always => {
                DiscriminantHints::Always
            }
            DiscriminantHintsDe::Never => {
                DiscriminantHints::Never
            }
            DiscriminantHintsDe::Fieldless => {
                DiscriminantHints::Fieldless
            }
        }
    }
}

#[derive(Deserialize)]
pub enum LifetimeElisionHintsDe {
    Always,
    Never,
    SkipTrivial,
}

impl From<LifetimeElisionHintsDe> for LifetimeElisionHints {
    fn from(v: LifetimeElisionHintsDe) -> Self {
        match v {
            LifetimeElisionHintsDe::Always => {
                LifetimeElisionHints::Always
            }
            LifetimeElisionHintsDe::Never => {
                LifetimeElisionHints::Never
            }
            LifetimeElisionHintsDe::SkipTrivial => {
                LifetimeElisionHints::SkipTrivial
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InlayFieldsToResolveDe {
    pub resolve_hint_tooltip: bool,
    pub resolve_label_command: bool,
    pub resolve_label_location: bool,
    pub resolve_label_tooltip: bool,
    pub resolve_text_edits: bool,
}

impl From<InlayFieldsToResolveDe> for InlayFieldsToResolve {
    fn from(c: InlayFieldsToResolveDe) -> Self {
        InlayFieldsToResolve {
            resolve_hint_tooltip: c.resolve_hint_tooltip,
            resolve_label_command: c.resolve_label_command,
            resolve_label_location: c.resolve_label_location,
            resolve_label_tooltip: c.resolve_label_tooltip,
            resolve_text_edits: c.resolve_text_edits,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GenericParameterHintsDe {
    pub type_hints: bool,
    pub lifetime_hints: bool,
    pub const_hints: bool,
}

impl From<GenericParameterHintsDe> for GenericParameterHints {
    fn from(c: GenericParameterHintsDe) -> Self {
        GenericParameterHints {
            type_hints: c.type_hints,
            lifetime_hints: c.lifetime_hints,
            const_hints: c.const_hints,
        }
    }
}

impl Default for InlayHintsConfigDe {
    fn default() -> Self {
        InlayHintsConfigDe {
            adjustment_hints: AdjustmentHintsDe::Never,
            adjustment_hints_disable_reborrows: true,
            adjustment_hints_hide_outside_unsafe: false,
            adjustment_hints_mode: AdjustmentHintsModeDe::Prefix,
            binding_mode_hints: false,
            chaining_hints: true,
            closing_brace_hints_min_lines: Some(16),
            closure_capture_hints: false,
            closure_return_type_hints:
                ClosureReturnTypeHintsDe::WithBlock,
            closure_style: ClosureStyleDe::RustAnalyzer,
            discriminant_hints: DiscriminantHintsDe::Fieldless,
            fields_to_resolve: InlayFieldsToResolveDe {
                resolve_hint_tooltip: true,
                resolve_label_command: true,
                resolve_label_location: true,
                resolve_label_tooltip: true,
                resolve_text_edits: true,
            },
            generic_parameter_hints: GenericParameterHintsDe {
                type_hints: true,
                lifetime_hints: true,
                const_hints: true,
            },
            hide_closure_initialization_hints: false,
            hide_closure_parameter_hints: false,
            hide_inferred_type_hints: false,
            hide_named_constructor_hints: false,
            implicit_drop_hints: false,
            implied_dyn_trait_hints: true,
            lifetime_elision_hints:
                LifetimeElisionHintsDe::Never,
            max_length: Some(256),
            param_names_for_lifetime_elision_hints: true,
            parameter_hints: true,
            parameter_hints_for_missing_arguments: true,
            range_exclusive_hints: true,
            render_colons: true,
            sized_bound: false,
            type_hints: true,
        }
    }
}

fn f() {
    let f = match std::fs::read_to_string("path") {
        Ok(s) => s,
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => {
                eprintln!(
                    "[ INFO ]: File not found, using default configuration"
                );
                String::from("")
            }
            _ => panic!(),
        },
    };

    let conf: InlayHintsConfig<'_> =
        serde_json::from_str::<InlayHintsConfigDe>(&f)
            .unwrap_or_default()
            .into();
}
