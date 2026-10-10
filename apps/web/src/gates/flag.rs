//! qni の測定変数 (`Measure>aliceX`) と条件付きゲート (`X<aliceX`)。
//!
//! qni の対応箇所:
//! - `packages/elements/src/quantum-circuit-element.ts` の `ifVariable` と
//!   `Measure` の分岐: `<` / `>` の後ろを trim した文字列を変数名にし、空なら
//!   通常のゲートとして読む。
//! - `packages/elements/src/gate-element-helpers.js` の `tI` / `tF`: 変数名が
//!   あるときだけ `<name` / `>name` を付けて書き出す。
//! - `packages/simulator/src/simulator.ts` の `runStep`: 測定は
//!   `flags[name] = (測定値 == 1)` を書き、条件付きゲートは `flags[name]` が
//!   true のときだけ適用する。一度も書かれていない変数は false。

use super::GateKind;

/// 測定結果を保持する変数の名前。前後の空白を除いた空でない文字列。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FlagName(String);

impl FlagName {
    /// qni と同じく前後の空白を除く。空になる名前は変数なしとみなす。
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then(|| Self(trimmed.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// ゲートと測定変数の関係。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum GateFlag {
    /// 測定ゲートが測定値 (0 / 1) をこの変数に書き込む (`Measure>name`)。
    Set(FlagName),
    /// 変数の値が 1 のときだけゲートを適用する (`X<name`)。
    If(FlagName),
}

impl GateFlag {
    /// `kind` がこの関係を持てるか。`Set` は測定ゲートだけ、`If` は qni の
    /// 回路 JSON で条件を書ける 1 量子ビットゲートだけが持てる。
    pub(crate) fn fits(&self, kind: GateKind) -> bool {
        match self {
            Self::Set(_) => kind == GateKind::Measurement,
            Self::If(_) => kind.is_ifable(),
        }
    }

    /// 回路上でゲートの上に表示するラベルの接頭辞。qni の CSS
    /// (`.operation-flaggable` / `.operation-ifable`) と同じく、測定ゲートは
    /// 変数名だけ、条件付きゲートは `if` と変数名を細いスペース (U+2009) で
    /// つないで表示する。
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

    /// 回路 JSON のトークンに付ける接尾辞 (`>name` / `<name`)。
    pub(crate) fn token_suffix(&self) -> String {
        match self {
            Self::Set(name) => format!(">{}", name.as_str()),
            Self::If(name) => format!("<{}", name.as_str()),
        }
    }

    /// `token` から変数の接尾辞を切り離し、残りのトークンと変数を返す。
    /// 接尾辞の形が `token` の種類に合わない (`Measure<a` / `X>a` など) ときは
    /// `None` を返し、呼び出し側で未知のトークンとして扱わせる。変数名が
    /// 空白だけのとき (`Measure>`) は qni と同じく変数なしの通常ゲートになる。
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
