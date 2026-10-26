use super::GateKind;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FlagName(String);

impl FlagName {
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then(|| Self(trimmed.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum GateFlag {
    Set(FlagName),
    If(FlagName),
}

impl GateFlag {
    pub(crate) fn fits(&self, kind: GateKind) -> bool {
        match self {
            Self::Set(_) => kind == GateKind::Measurement,
            Self::If(_) => kind.is_ifable(),
        }
    }

    pub(crate) fn label_prefix(&self) -> Option<&'static str> {
        match self {
            Self::Set(_) => None,
            Self::If(_) => Some("if"),
        }
    }

    pub(crate) fn name(&self) -> &FlagName {
        match self {
            Self::Set(name) | Self::If(name) => name,
        }
    }

    pub(crate) fn token_suffix(&self) -> String {
        match self {
            Self::Set(name) => format!(">{}", name.as_str()),
            Self::If(name) => format!("<{}", name.as_str()),
        }
    }

    pub(crate) fn split_token(token: &str) -> Option<(&str, Option<Self>)> {
        if let Some(name) = token.strip_prefix("Measure>") {
            return Some(("Measure", FlagName::parse(name).map(Self::Set)));
        }
        let Some((base, name)) = token.split_once('<') else {
            return Some((token, None));
        };
        GateKind::from_url_token(base)
            .is_some_and(GateKind::is_ifable)
            .then(|| (base, FlagName::parse(name).map(Self::If)))
    }
}

impl GateKind {
    /// qni の回路 JSON で `<name` の条件を書けるゲート。qni の要素では
    /// P / Rx / Ry / Rz も `if` 属性を持つが、JSON の読み書き
    /// (`angleParameter` / `tA`) が角度しか扱わないため条件を保存できない。
    pub(crate) fn is_ifable(self) -> bool {
        matches!(
            self,
            GateKind::H
                | GateKind::X
                | GateKind::Y
                | GateKind::Z
                | GateKind::SqrtX
                | GateKind::S
                | GateKind::SDagger
                | GateKind::T
                | GateKind::TDagger
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(raw: &str) -> FlagName {
        FlagName::parse(raw).expect("test flag name must be non-empty")
    }

    #[test]
    fn measure_token_sets_named_flag() {
        assert_eq!(
            GateFlag::split_token("Measure>aliceX"),
            Some(("Measure", Some(GateFlag::Set(name("aliceX")))))
        );
    }

    #[test]
    fn conditional_token_reads_named_flag() {
        assert_eq!(
            GateFlag::split_token("X<aliceX"),
            Some(("X", Some(GateFlag::If(name("aliceX")))))
        );
    }

    #[test]
    fn dagger_conditional_token_keeps_the_dagger_base() {
        assert_eq!(
            GateFlag::split_token("T†<f"),
            Some(("T†", Some(GateFlag::If(name("f")))))
        );
    }

    #[test]
    fn flag_name_is_trimmed() {
        assert_eq!(
            GateFlag::split_token("H< bobH "),
            Some(("H", Some(GateFlag::If(name("bobH")))))
        );
    }

    #[test]
    fn blank_flag_name_reads_as_a_plain_gate() {
        assert_eq!(GateFlag::split_token("Measure> "), Some(("Measure", None)));
    }

    #[test]
    fn plain_token_has_no_flag() {
        assert_eq!(GateFlag::split_token("H"), Some(("H", None)));
    }

    #[test]
    fn condition_on_a_measurement_is_rejected() {
        assert_eq!(GateFlag::split_token("Measure<a"), None);
    }

    #[test]
    fn condition_on_a_parametric_gate_is_rejected() {
        assert_eq!(GateFlag::split_token("Rx(π_2)<a"), None);
    }

    #[test]
    fn conditional_label_starts_with_if() {
        assert_eq!(GateFlag::If(name("aliceX")).label_prefix(), Some("if"));
    }

    #[test]
    fn measurement_label_is_only_the_flag_name() {
        assert_eq!(GateFlag::Set(name("aliceX")).label_prefix(), None);
    }

    #[test]
    fn set_flag_fits_only_measurements() {
        assert!(!GateFlag::Set(name("a")).fits(GateKind::X));
    }

    #[test]
    fn if_flag_does_not_fit_swap() {
        assert!(!GateFlag::If(name("a")).fits(GateKind::Swap));
    }
}
