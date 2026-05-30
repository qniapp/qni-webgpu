//! Domain model for the local circuit library.

use std::fmt;

use serde::{Deserialize, Serialize};

pub const EMPTY_CIRCUIT_JSON: &str = r#"{"cols":[]}"#;
const GROVER_SEARCH_JSON: &str = concat!(
    r#"{"cols":["#,
    r#"["X","X","X","X","X"],["H","H","H","H","H"],["Probability5"],"#,
    r#"["Z","•","◦","•","•"],["H","H","H","H",1],["•","•","•","•","X"],["H","H","H","H",1],["Probability5"],"#,
    r#"["Z","•","◦","•","•"],["H","H","H","H",1],["•","•","•","•","X"],["H","H","H","H",1],["Probability5"],"#,
    r#"["Z","•","◦","•","•"],["H","H","H","H",1],["•","•","•","•","X"],["H","H","H","H",1],["Probability5"],"#,
    r#"["Z","•","◦","•","•"],["H","H","H","H",1],["•","•","•","•","X"],["H","H","H","H",1],["Probability5"]"#,
    r#"]}"#,
);

/// QFT を H と制御位相回転へ分解した 4 量子ビット回路。ネイティブ `QFT4`
/// ゲートを教材として展開したもので、シミュレータの `linearize_qft` が
/// `QFT4` を変換する手順そのもの（先頭でビット反転 SWAP を掛けてから、各ビット
/// に H を当て、より上位のビットを制御とする制御位相 π/2^j を掛ける。
/// j=1→π/2, j=2→π/4, j=3→π/8）を回路として書き下した。先頭の SWAP で入力を
/// ビット反転してからラダーを通すことで、真の対称離散フーリエ変換（Quirk の
/// FourierTransformGates）になる。`QFT4` と数学的に等価であることは Web アプリ側の
/// `decomposed_qft4_matches_native_qft4_ops` テストで保証する。
pub const QFT4_DECOMPOSED_JSON: &str = concat!(
    r#"{"cols":["#,
    r#"["Swap",1,1,"Swap"],"#,
    r#"[1,"Swap","Swap"],"#,
    r#"["H"],"#,
    r#"["P(π_2)","•"],"#,
    r#"["P(π_4)",1,"•"],"#,
    r#"["P(π_8)",1,1,"•"],"#,
    r#"[1,"H"],"#,
    r#"[1,"P(π_2)","•"],"#,
    r#"[1,"P(π_4)",1,"•"],"#,
    r#"[1,1,"H"],"#,
    r#"[1,1,"P(π_2)","•"],"#,
    r#"[1,1,1,"H"]"#,
    r#"]}"#,
);

/// 対称性の破れ（Symmetry Breaking）4 量子ビット回路。上位 2 本（q0,q1）と下位 2 本
/// （q2,q3）にまったく同じ H・CNOT で Bell ペアを作り、q1↔q3 の SWAP で結合、再度
/// CNOT、反制御つき √X を経て測定する。同一・対称に作った 2 つのサブシステムが必ず
/// 反相関（不一致）になる様子を見せる。Quirk の symmetryBreakingLink を移植したもので、
/// 確率表示の ID を Chance → Probability に揃え、注釈用の恒等ラベルは除いてある。
/// 解説は docs/implementation/symmetry-breaking.html。
///
/// 制御を伴う操作（2 つの CNOT 層と反制御つき √X）は列を分ける。同じ列に複数の制御を
/// 置くと qni では列内の全制御が全ターゲットに掛かり、独立した 2 つの操作ではなく
/// 多重制御になってしまうため。制御を持たない H と最後の √X は同じ列にまとめてよい。
const SYMMETRY_BREAKING_JSON: &str = concat!(
    r#"{"cols":["#,
    r#"["H",1,"H"],"#,
    r#"["•","X"],"#,
    r#"[1,1,"•","X"],"#,
    r#"[1,"Swap",1,"Swap"],"#,
    r#"["•","X"],"#,
    r#"[1,1,"•","X"],"#,
    r#"["X^½","◦"],"#,
    r#"[1,1,"X^½","◦"],"#,
    r#"[1,"X^½",1,"X^½"],"#,
    r#"["Measure","Measure","Measure","Measure"]"#,
    r#"]}"#,
);

/// 遅延選択量子消しゴム（Delayed Choice Quantum Eraser）の 9 量子ビット回路。
/// q0=選択（経路情報を消すか）、q1=経路（光子）、q2–q8=スクリーン（7 量子ビット）。
/// 経路情報をスクリーンへ刻んで QFT で位置分布を作り、q0 の選択と controlled-√X（消しゴム）で
/// 経路情報を消すか決め、末尾の条件付き確率表示ブロックが「干渉縞（消去時）／のっぺり
/// （非消去時）」を仕分けて見せる。
///
/// 測定を置かないコヒーレント版で実現している。qni の測定は状態を 1 サンプルに潰すため、
/// Quirk のようなアンサンブルの干渉縞が 1 画面では出ない。そこで測定を省き、コヒーレントな
/// 状態への条件付き確率表示で 4 通りを同時に見せる（Quirk が「測定して仕分ける」分布を、
/// 状態を潰さずに提示）。この形では遅延（スクリーンを先に測定）の演出は省いている。
///
/// which-path のマークは Quirk と同じ q4 に置く。qni も上のワイヤを最下位ビット
/// （LSB）として数える q0=LSB 規約になり、Quirk のリトルエンディアンと一致したため、
/// Quirk が q4（QFT レジスタの上から 3 番目）に置くマークをそのまま q4 に置けば
/// 干渉縞が 4 本になり Quirk の見た目と一致する。解説は
/// docs/implementation/delayed-choice-eraser.html。
const DELAYED_CHOICE_ERASER_JSON: &str = concat!(
    r#"{"cols":["#,
    r#"[1,"H"],"#,
    r#"[1,"•",1,1,"X"],"#,
    r#"[1,1,"QFT7"],"#,
    r#"[1,1,"Probability7"],"#,
    r#"["H"],"#,
    r#"["•","X^½"],"#,
    r#"["◦","◦","Probability7"],"#,
    r#"["◦","•","Probability7"],"#,
    r#"["•","◦","Probability7"],"#,
    r#"["•","•","Probability7"]"#,
    r#"]}"#,
);

/// 量子テレポーテーション 3 量子ビット回路。q0=メッセージ、q1=アリス、q2=ボブ。
/// もつれ（Bell ペア）と 2 ビットの古典情報で未知の状態を q0 から q2 へ移す。アリスの
/// ベル測定（CNOT・H・測定）で q0・q1 を潰し、ボブが測定結果で X / Z 補正をかけると、
/// ランダムな測定結果によらず q2 が決定的にメッセージと一致する（qni の硬測定が正しく働く例。
/// 測定で状態が潰れることをそのまま使う点で遅延選択量子消しゴムと対照的）。転送前後を
/// 2 つのブロッホ球表示ブロックで見比べる。解説は
/// docs/implementation/quantum-teleportation.html。
const TELEPORTATION_JSON: &str = concat!(
    r#"{"cols":["#,
    r#"[1,"H"],"#,
    r#"[1,"•","X"],"#,
    r#"["H"],"#,
    r#"["T"],"#,
    r#"["Bloch"],"#,
    r#"["•","X"],"#,
    r#"["H"],"#,
    r#"["Measure","Measure"],"#,
    r#"[1,"•","X"],"#,
    r#"["•",1,"Z"],"#,
    r#"[1,1,"Bloch"]"#,
    r#"]}"#,
);

/// 超高密度符号化（Superdense Coding）6 量子ビット回路。q0・q1=送信ビット、q2=アリス、
/// q3・q4=ベルペアを作って配る量子ビット、q5=ボブ。ベルペアを生成して Swap でアリスとボブへ
/// 配り、Z・X でメッセージ「11」を符号化、Swap で送信し、ボブのベル測定（CNOT・H・測定）で
/// 2 ビットを復号する。テレポーテーションの双対で、こちらも qni の硬測定が正しく働く。解説は
/// docs/implementation/superdense-coding.html。
const SUPERDENSE_CODING_JSON: &str = concat!(
    r#"{"cols":["#,
    r#"[1,1,1,"H"],"#,
    r#"[1,1,1,"•","X"],"#,
    r#"[1,1,"Swap","Swap"],"#,
    r#"[1,1,1,1,"Swap","Swap"],"#,
    r#"["|1>","|1>"],"#,
    r#"["•",1,"Z"],"#,
    r#"[1,"•","X"],"#,
    r#"[1,1,"Swap",1,"Swap"],"#,
    r#"[1,1,1,1,"•","X"],"#,
    r#"[1,1,1,1,"H"],"#,
    r#"[1,1,1,1,"Measure","Measure"]"#,
    r#"]}"#,
);

/// 可逆加算（Reversible Addition）10 量子ビット回路。q0-q4=A レジスタ、q5-q9=B レジスタ
/// （下位ビットが上のワイヤ）。Quirk の Swap ネット構成で、A を保ったまま B に A を足し込む
/// （B += A、5 ビット同士の mod 32）。入力は A=5・B=3 に固定し、B を 8 にする。多ターゲット制御 X
/// （X の壁）と制御 Swap（Fredkin）を使う。どちらも GPU シェーダが対応済みで新ゲートは不要。解説は
/// docs/implementation/reversible-addition.html。
const REVERSIBLE_ADDITION_JSON: &str = concat!(
    r#"{"cols":["#,
    r#"["X",1,"X",1,1,"X","X"],"#,
    r#"["X","X","X","X","•","X","X","X","X","X"],"#,
    r#"[1,1,1,1,"•","X"],"#,
    r#"["Swap",1,1,1,"Swap","•"],"#,
    r#"[1,1,1,1,"•",1,"X"],"#,
    r#"[1,"Swap",1,1,"Swap",1,"•"],"#,
    r#"[1,1,1,1,"•",1,1,"X"],"#,
    r#"[1,1,"Swap",1,"Swap",1,1,"•"],"#,
    r#"[1,1,1,1,"•",1,1,1,"X"],"#,
    r#"[1,1,1,"Swap","Swap",1,1,1,"•"],"#,
    r#"[1,1,1,1,"•",1,1,1,1,"X"],"#,
    r#"[1,1,1,"Swap","Swap",1,1,1,"•"],"#,
    r#"[1,1,1,"•",1,1,1,1,"X"],"#,
    r#"[1,1,"Swap",1,"Swap",1,1,"•"],"#,
    r#"[1,1,"•",1,1,1,1,"X"],"#,
    r#"[1,"Swap",1,1,"Swap",1,"•"],"#,
    r#"[1,"•",1,1,1,1,"X"],"#,
    r#"["Swap",1,1,1,"Swap","•"],"#,
    r#"["•",1,1,1,1,"X"],"#,
    r#"["X","X","X","X","•","X","X","X","X","X"]"#,
    r#"]}"#,
);

/// 回路ライブラリのエントリ識別子。常に空でない文字列を保持する値オブジェクト。
///
/// 保存名や回路 JSON と同じ `String` で取り違えないよう型で区別し、生成時に
/// 「空でない」不変条件を強制する。JSON では `#[serde(try_from / into)]` により
/// 従来どおり素の文字列として読み書きし、逆シリアライズの時点で検証する。
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CircuitId(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CircuitIdError {
    Empty,
}

impl fmt::Display for CircuitIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("circuit id must not be empty"),
        }
    }
}

impl CircuitId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, CircuitIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CircuitIdError::Empty);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// クレート内部で生成する識別子（サンプルの固定文字列・採番結果）専用。
    /// これらは構造上空にならないため、検証失敗は不変条件の破れとして panic させる。
    fn from_known(value: impl Into<String>) -> Self {
        Self::try_new(value).expect("internally generated circuit id must not be empty")
    }
}

impl TryFrom<String> for CircuitId {
    type Error = CircuitIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<CircuitId> for String {
    fn from(id: CircuitId) -> Self {
        id.0
    }
}

const DEFAULT_CIRCUIT_NAME_PREFIX: &str = "Circuit ";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CircuitEntry {
    pub id: CircuitId,
    pub name: String,
    pub circuit_json: String,
    pub updated_at: u64,
    pub origin: CircuitOrigin,
}

impl CircuitEntry {
    pub fn sample(id: &str, name: &str, circuit_json: &str, updated_at: u64) -> Self {
        Self {
            id: CircuitId::from_known(id),
            name: name.to_owned(),
            circuit_json: circuit_json.to_owned(),
            updated_at,
            origin: CircuitOrigin::Sample {
                origin_id: CircuitId::from_known(id),
            },
        }
    }

    pub fn user(
        id: CircuitId,
        name: String,
        circuit_json: String,
        updated_at: u64,
        locked: bool,
    ) -> Self {
        Self {
            id,
            name,
            circuit_json,
            updated_at,
            origin: CircuitOrigin::User { locked },
        }
    }

    pub fn kind(&self) -> CircuitKind {
        match self.origin {
            CircuitOrigin::Sample { .. } => CircuitKind::Example,
            CircuitOrigin::User { .. } => CircuitKind::My,
        }
    }

    pub fn locked(&self) -> bool {
        match self.origin {
            CircuitOrigin::Sample { .. } => true,
            CircuitOrigin::User { locked } => locked,
        }
    }

    pub fn is_sample(&self) -> bool {
        matches!(self.origin, CircuitOrigin::Sample { .. })
    }

    pub fn is_user(&self) -> bool {
        matches!(self.origin, CircuitOrigin::User { .. })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CircuitOrigin {
    Sample { origin_id: CircuitId },
    User { locked: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CircuitKind {
    Example,
    My,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CircuitLibrary {
    pub entries: Vec<CircuitEntry>,
    pub active_id: CircuitId,
}

impl Default for CircuitLibrary {
    fn default() -> Self {
        Self::seed()
    }
}

impl CircuitLibrary {
    pub fn seed() -> Self {
        let entries = sample_entries(now_millis());
        Self {
            active_id: entries[0].id.clone(),
            entries,
        }
    }

    pub fn from_entries(entries: Vec<CircuitEntry>, active_id: CircuitId) -> Self {
        let mut library = Self { entries, active_id };
        library.ensure_non_empty();
        if !library
            .entries
            .iter()
            .any(|entry| entry.id == library.active_id)
        {
            library.active_id = library.entries[0].id.clone();
        }
        library
    }

    pub fn migrate_v1_entries(entries: Vec<CircuitEntry>, active_id: Option<CircuitId>) -> Self {
        migrate_v1_entries(entries, active_id)
    }

    pub fn active_index(&self) -> usize {
        self.entries
            .iter()
            .position(|entry| entry.id == self.active_id)
            .unwrap_or(0)
    }

    pub fn active(&self) -> &CircuitEntry {
        &self.entries[self.active_index()]
    }

    pub fn active_kind(&self) -> CircuitKind {
        self.active().kind()
    }

    pub fn active_locked(&self) -> bool {
        self.active().locked()
    }

    pub fn entry_locked_by_id(&self, id: &CircuitId) -> bool {
        self.entries
            .iter()
            .find(|entry| entry.id == *id)
            .is_some_and(CircuitEntry::locked)
    }

    pub fn update_active(&mut self, circuit_json: String) {
        if self.active_locked() {
            return;
        }
        self.update_active_unchecked(circuit_json);
    }

    pub fn update_active_unchecked(&mut self, circuit_json: String) {
        let active_id = self.active_id.clone();
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == active_id) {
            entry.circuit_json = circuit_json;
            entry.updated_at = now_millis();
        }
    }

    pub fn set_active(&mut self, id: CircuitId) -> &CircuitEntry {
        if self.entries.iter().any(|entry| entry.id == id) {
            self.active_id = id;
        }
        self.active()
    }

    pub fn set_active_index(&mut self, index: usize) -> &CircuitEntry {
        if let Some(entry) = self.entries.get(index) {
            self.active_id = entry.id.clone();
        }
        self.active()
    }

    pub fn rename(&mut self, id: &CircuitId, name: &str) {
        if self.entry_locked_by_id(id) {
            return;
        }
        self.rename_unchecked(id, name);
    }

    pub fn rename_unchecked(&mut self, id: &CircuitId, name: &str) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == *id) {
            entry.name = trimmed.to_owned();
            entry.updated_at = now_millis();
        }
    }

    pub fn duplicate(&mut self, index: usize) -> Option<&CircuitEntry> {
        self.duplicate_at_index(index)?;
        Some(self.active())
    }

    /// Insert a copy of the active entry into the My section; switch active to
    /// the new unlocked User entry and bump its timestamp. Copy names follow
    /// the picker/toolbar contract: "Name (copy)", then "Name (copy 2)", …
    pub fn duplicate_active(&mut self) -> CircuitId {
        let index = self.active_index();
        self.duplicate_at_index(index)
            .expect("active circuit entry should always exist")
    }

    pub fn move_up(&mut self, index: usize) {
        let Some((kind, slot)) = self.section_slot_for_index(index) else {
            return;
        };
        if slot == 0 {
            return;
        }
        self.move_section_entry_to_slot(index, slot - 1, kind);
    }

    pub fn move_down(&mut self, index: usize) {
        let Some((kind, slot)) = self.section_slot_for_index(index) else {
            return;
        };
        if slot + 1 >= self.section_indices(kind).len() {
            return;
        }
        self.move_section_entry_to_slot(index, slot + 1, kind);
    }

    #[allow(dead_code)]
    pub fn reorder(&mut self, src: usize, target: usize) {
        if src >= self.entries.len() || target == src || target == src + 1 {
            return;
        }
        let entry = self.entries.remove(src);
        let adjusted = if target > src { target - 1 } else { target };
        self.entries.insert(adjusted.min(self.entries.len()), entry);
        self.bump_updated_at();
    }

    pub fn move_to_slot(&mut self, src: usize, slot: usize) {
        let Some((kind, source_slot)) = self.section_slot_for_index(src) else {
            return;
        };
        let section_len = self.section_indices(kind).len();
        if slot >= self.entries.len() || src == slot || section_len == 0 {
            return;
        }
        let target_slot = self
            .section_slot_for_index(slot)
            .and_then(|(target_kind, target_slot)| (target_kind == kind).then_some(target_slot))
            .unwrap_or_else(|| source_slot.min(section_len.saturating_sub(1)));
        self.move_section_entry_to_slot(src, target_slot, kind);
    }

    pub fn move_user_to_slot(&mut self, src_index: usize, target_user_slot: usize) {
        self.move_section_entry_to_slot(src_index, target_user_slot, CircuitKind::My);
    }

    pub fn swap_adjacent(&mut self, a: usize, b: usize) {
        debug_assert!(a.abs_diff(b) == 1);
        if a < self.entries.len()
            && b < self.entries.len()
            && a.abs_diff(b) == 1
            && self.entries[a].is_user()
            && self.entries[b].is_user()
        {
            self.entries.swap(a, b);
        }
    }

    pub fn bump_updated_at(&mut self) {
        let active_id = self.active_id.clone();
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == active_id) {
            entry.updated_at = now_millis();
        }
    }

    pub fn delete(&mut self, index: usize) -> Option<&CircuitEntry> {
        let id = self.entries.get(index)?.id.clone();
        self.delete_by_id(&id)
    }

    pub fn delete_by_id(&mut self, id: &CircuitId) -> Option<&CircuitEntry> {
        if self.entries.len() <= 1 || self.entry_locked_by_id(id) {
            return None;
        }
        let index = self.entries.iter().position(|entry| entry.id == *id)?;
        self.entries.remove(index);
        if self.active_id == *id {
            self.active_id = self
                .entries
                .iter()
                .find(|entry| entry.is_user())
                .or_else(|| self.entries.first())
                .map(|entry| entry.id.clone())
                .expect("library keeps at least one entry after a guarded delete");
        }
        Some(self.active())
    }

    pub fn create_new(&mut self) -> &CircuitEntry {
        let entry = CircuitEntry::user(
            self.fresh_id("circuit"),
            self.next_default_circuit_name(None),
            EMPTY_CIRCUIT_JSON.to_owned(),
            now_millis(),
            false,
        );
        let id = entry.id.clone();
        self.entries.push(entry);
        self.set_active(id)
    }

    pub fn set_active_current_circuit(&mut self, circuit_json: String) {
        self.set_active_current_circuit_with_lock_policy(circuit_json, false);
    }

    pub fn set_active_current_circuit_preserving_locked(&mut self, circuit_json: String) {
        self.set_active_current_circuit_with_lock_policy(circuit_json, true);
    }

    pub fn toggle_active_lock(&mut self) -> bool {
        let active_id = self.active_id.clone();
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == active_id) {
            if let CircuitOrigin::User { locked } = &mut entry.origin {
                *locked = !*locked;
                entry.updated_at = now_millis();
                return true;
            }
        }
        false
    }

    pub fn migrate_legacy_default_names(&mut self) -> bool {
        let mut changed = false;
        let now = now_millis();
        for index in 0..self.entries.len() {
            if self.entries[index].is_user()
                && self.entries[index].name == "Untitled"
                && is_auto_generated_circuit_id(self.entries[index].id.as_str())
            {
                let id = self.entries[index].id.clone();
                self.entries[index].name = self.next_default_circuit_name(Some(id.as_str()));
                self.entries[index].updated_at = now;
                changed = true;
            }
        }
        changed
    }

    /// 永続化された組み込みサンプルを現在のコード定義（`sample_entries`）へ同期する。
    ///
    /// 組み込みサンプルの JSON や名前をコード側で更新しても、初回シード後に保存された
    /// ライブラリは取り残される。ロード時にこの調整を掛けることで、同じ `id` を持つ
    /// 既存サンプルの name / circuit_json をコード定義へ更新し、保存版に無いサンプルは
    /// 追加する。ユーザー回路（`CircuitOrigin::User`）には一切触れない。既存サンプルの
    /// `updated_at` は据え置く（表示はセクション別なので並び順には影響しない）。
    /// 変更があれば true を返す。
    pub fn reconcile_samples(&mut self) -> bool {
        let mut changed = false;
        for canonical in sample_entries(now_millis()) {
            if let Some(existing) = self
                .entries
                .iter_mut()
                .find(|entry| entry.is_sample() && entry.id == canonical.id)
            {
                if existing.name != canonical.name
                    || existing.circuit_json != canonical.circuit_json
                {
                    existing.name = canonical.name;
                    existing.circuit_json = canonical.circuit_json;
                    changed = true;
                }
            } else {
                self.entries.push(canonical);
                changed = true;
            }
        }
        changed
    }

    pub fn resolve_startup_url_payload(&mut self, url_json: String) -> bool {
        if self.active().circuit_json == url_json {
            return false;
        }
        if let Some(sample_id) = self.find_canonical_sample(&url_json) {
            let changed = self.active_id != sample_id;
            self.active_id = sample_id;
            return changed;
        }
        if let Some(user_id) = self.find_user_entry_with_json(&url_json) {
            let changed = self.active_id != user_id;
            self.active_id = user_id;
            return changed;
        }
        let old_active = self.active_id.clone();
        self.set_active_current_circuit_preserving_locked(url_json);
        self.active_id != old_active
    }

    pub fn find_canonical_sample(&self, circuit_json: &str) -> Option<CircuitId> {
        sample_entries(0)
            .into_iter()
            .find(|entry| entry.circuit_json == circuit_json)
            .map(|entry| entry.id)
    }

    pub fn find_user_entry_with_json(&self, circuit_json: &str) -> Option<CircuitId> {
        self.entries
            .iter()
            .find(|entry| entry.is_user() && entry.circuit_json == circuit_json)
            .map(|entry| entry.id.clone())
    }

    pub fn user_indices(&self) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| entry.is_user().then_some(index))
            .collect()
    }

    pub fn sample_indices(&self) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| entry.is_sample().then_some(index))
            .collect()
    }

    pub fn user_slot_for_index(&self, index: usize) -> Option<usize> {
        self.user_indices()
            .iter()
            .position(|user_index| *user_index == index)
    }

    pub fn section_slot_for_index(&self, index: usize) -> Option<(CircuitKind, usize)> {
        let kind = self.entries.get(index)?.kind();
        self.section_indices(kind)
            .iter()
            .position(|entry_index| *entry_index == index)
            .map(|slot| (kind, slot))
    }

    pub fn to_test_json(&self) -> String {
        let entries = self
            .entries
            .iter()
            .map(|entry| {
                format!(
                    r#"{{"id":"{}","name":"{}","circuit_json":"{}","updated_at":{},"locked":{},"origin":{}}}"#,
                    json_escape(entry.id.as_str()),
                    json_escape(&entry.name),
                    json_escape(&entry.circuit_json),
                    entry.updated_at,
                    entry.locked(),
                    origin_test_json(&entry.origin),
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"entries":[{}],"active_id":"{}","active_locked":{},"active_kind":"{}"}}"#,
            entries,
            json_escape(self.active_id.as_str()),
            self.active_locked(),
            match self.active_kind() {
                CircuitKind::Example => "example",
                CircuitKind::My => "my",
            },
        )
    }

    fn duplicate_at_index(&mut self, index: usize) -> Option<CircuitId> {
        let source = self.entries.get(index)?.clone();
        let entry = CircuitEntry::user(
            self.fresh_id("circuit"),
            self.unique_copy_name(&source.name),
            source.circuit_json,
            now_millis(),
            false,
        );
        let id = entry.id.clone();
        self.entries.push(entry);
        self.active_id = id.clone();
        self.bump_updated_at();
        Some(id)
    }

    fn unique_copy_name(&self, source_name: &str) -> String {
        let root = copy_name_root(source_name);
        let first = format!("{root} (copy)");
        if !self.entries.iter().any(|entry| entry.name == first) {
            return first;
        }
        let mut suffix = 2;
        loop {
            let candidate = format!("{root} (copy {suffix})");
            if !self.entries.iter().any(|entry| entry.name == candidate) {
                return candidate;
            }
            suffix += 1;
        }
    }

    fn ensure_non_empty(&mut self) {
        if self.entries.is_empty() {
            self.entries.push(CircuitEntry::user(
                CircuitId::from_known("circuit-1"),
                "Circuit 1".to_owned(),
                EMPTY_CIRCUIT_JSON.to_owned(),
                now_millis(),
                false,
            ));
        }
    }

    fn next_default_circuit_name(&self, excluding_id: Option<&str>) -> String {
        let next_index = self
            .entries
            .iter()
            .filter(|entry| excluding_id != Some(entry.id.as_str()))
            .filter_map(|entry| default_circuit_number(&entry.name))
            .max()
            .unwrap_or(0)
            + 1;
        format!("{DEFAULT_CIRCUIT_NAME_PREFIX}{next_index}")
    }

    fn fresh_id(&self, prefix: &str) -> CircuitId {
        let mut index = self.entries.len() + 1;
        loop {
            let id = format!("{prefix}-{index}");
            if !self.entries.iter().any(|entry| entry.id.as_str() == id) {
                return CircuitId::from_known(id);
            }
            index += 1;
        }
    }

    fn set_active_current_circuit_with_lock_policy(
        &mut self,
        circuit_json: String,
        preserve_locked_current: bool,
    ) {
        let mut id = CircuitId::from_known("current");
        if preserve_locked_current && self.entry_locked_by_id(&id) {
            id = self.fresh_id("current");
        }
        let entry = CircuitEntry::user(
            id.clone(),
            self.next_default_circuit_name(Some(id.as_str())),
            circuit_json,
            now_millis(),
            false,
        );
        if let Some(existing) = self.entries.iter_mut().find(|entry| entry.id == id) {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
        self.active_id = id;
        self.normalize_sample_user_order();
    }

    fn normalize_sample_user_order(&mut self) {
        let samples = self.section_entries(CircuitKind::Example);
        let users = self.section_entries(CircuitKind::My);
        self.entries = samples.into_iter().chain(users).collect();
    }

    fn move_section_entry_to_slot(
        &mut self,
        src_index: usize,
        target_slot: usize,
        kind: CircuitKind,
    ) {
        let section_indices = self.section_indices(kind);
        let Some(source_slot) = section_indices.iter().position(|index| *index == src_index) else {
            return;
        };
        if target_slot >= section_indices.len() || source_slot == target_slot {
            return;
        }
        let mut samples = self.section_entries(CircuitKind::Example);
        let mut users = self.section_entries(CircuitKind::My);
        let section = match kind {
            CircuitKind::Example => &mut samples,
            CircuitKind::My => &mut users,
        };
        let moved = section.remove(source_slot);
        section.insert(target_slot, moved);
        self.entries = samples.into_iter().chain(users).collect();
    }

    fn section_indices(&self, kind: CircuitKind) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| (entry.kind() == kind).then_some(index))
            .collect()
    }

    fn section_entries(&self, kind: CircuitKind) -> Vec<CircuitEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.kind() == kind)
            .cloned()
            .collect()
    }
}

fn sample_entries(updated_at: u64) -> Vec<CircuitEntry> {
    vec![
        CircuitEntry::sample(
            "bell",
            "Bell state",
            r#"{"cols":[["H"],["•","X"]]}"#,
            updated_at,
        ),
        CircuitEntry::sample(
            "ghz",
            "GHZ state",
            r#"{"cols":[["H"],["•","X"],["•",1,"X"]]}"#,
            updated_at,
        ),
        CircuitEntry::sample(
            "quantum-teleportation",
            "Quantum Teleportation",
            TELEPORTATION_JSON,
            updated_at,
        ),
        CircuitEntry::sample(
            "superdense-coding",
            "Superdense Coding",
            SUPERDENSE_CODING_JSON,
            updated_at,
        ),
        CircuitEntry::sample("qft-4", "QFT 4-qubit", QFT4_DECOMPOSED_JSON, updated_at),
        CircuitEntry::sample(
            "symmetry-breaking",
            "Symmetry Breaking",
            SYMMETRY_BREAKING_JSON,
            updated_at,
        ),
        grover_search_entry(updated_at),
        CircuitEntry::sample(
            "reversible-addition",
            "Reversible Addition",
            REVERSIBLE_ADDITION_JSON,
            updated_at,
        ),
        CircuitEntry::sample(
            "delayed-choice-eraser",
            "Delayed Choice Eraser",
            DELAYED_CHOICE_ERASER_JSON,
            updated_at,
        ),
    ]
}

fn grover_search_entry(updated_at: u64) -> CircuitEntry {
    CircuitEntry::sample(
        "grover-search",
        "Grover Search",
        GROVER_SEARCH_JSON,
        updated_at,
    )
}

fn migrate_v1_entries(entries: Vec<CircuitEntry>, active_id: Option<CircuitId>) -> CircuitLibrary {
    let mut migrated = CircuitLibrary::seed();
    let mut active_remap = Vec::<(CircuitId, CircuitId)>::new();
    let samples = sample_entries(0);
    for entry in entries {
        if let Some(sample) = samples.iter().find(|sample| sample.id == entry.id) {
            if sample.name == entry.name && sample.circuit_json == entry.circuit_json {
                active_remap.push((entry.id.clone(), sample.id.clone()));
                continue;
            }
            let next_id = unique_id(
                &migrated.entries,
                &format!("{}-user-edit", entry.id.as_str()),
                "-",
            );
            let next_name = unique_edited_name(&migrated.entries, &entry.name);
            active_remap.push((entry.id.clone(), next_id.clone()));
            migrated.entries.push(CircuitEntry::user(
                next_id,
                next_name,
                entry.circuit_json,
                entry.updated_at,
                false,
            ));
        } else {
            let next_id = unique_id(&migrated.entries, entry.id.as_str(), "-");
            active_remap.push((entry.id.clone(), next_id.clone()));
            migrated.entries.push(CircuitEntry::user(
                next_id,
                entry.name,
                entry.circuit_json,
                entry.updated_at,
                false,
            ));
        }
    }
    if let Some(active_id) = active_id {
        if let Some((_, remapped)) = active_remap.iter().find(|(old, _)| old == &active_id) {
            migrated.active_id = remapped.clone();
        }
    }
    if !migrated
        .entries
        .iter()
        .any(|entry| entry.id == migrated.active_id)
    {
        migrated.active_id = migrated.entries[0].id.clone();
    }
    migrated
}

fn unique_id(entries: &[CircuitEntry], preferred: &str, separator: &str) -> CircuitId {
    if !entries.iter().any(|entry| entry.id.as_str() == preferred) {
        return CircuitId::from_known(preferred);
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{preferred}{separator}{suffix}");
        if !entries.iter().any(|entry| entry.id.as_str() == candidate) {
            return CircuitId::from_known(candidate);
        }
        suffix += 1;
    }
}

fn unique_edited_name(entries: &[CircuitEntry], name: &str) -> String {
    let first = format!("{name} (edited)");
    if !entries.iter().any(|entry| entry.name == first) {
        return first;
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{name} (edited {suffix})");
        if !entries.iter().any(|entry| entry.name == candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

fn is_auto_generated_circuit_id(id: &str) -> bool {
    if id == "current" {
        return true;
    }
    id.strip_prefix("circuit-")
        .is_some_and(|suffix| suffix.parse::<usize>().is_ok())
}

fn default_circuit_number(name: &str) -> Option<usize> {
    let number = name.strip_prefix(DEFAULT_CIRCUIT_NAME_PREFIX)?;
    if number.is_empty() || !number.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    match number.parse::<usize>() {
        Ok(0) | Err(_) => None,
        Ok(number) => Some(number),
    }
}

fn copy_name_root(name: &str) -> &str {
    if let Some(root) = name.strip_suffix(" (copy)") {
        return root;
    }
    if let Some((root, suffix)) = name.rsplit_once(" (copy ") {
        if let Some(number) = suffix.strip_suffix(')') {
            if number.parse::<usize>().is_ok() {
                return root;
            }
        }
    }
    name
}

fn origin_test_json(origin: &CircuitOrigin) -> String {
    match origin {
        CircuitOrigin::Sample { origin_id } => format!(
            r#"{{"kind":"sample","origin_id":"{}"}}"#,
            json_escape(origin_id.as_str())
        ),
        CircuitOrigin::User { locked } => format!(r#"{{"kind":"user","locked":{locked}}}"#),
    }
}

fn json_escape(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(target_arch = "wasm32")]
pub fn now_millis() -> u64 {
    js_sys::Date::now().max(0.0) as u64
}

#[cfg(not(target_arch = "wasm32"))]
pub fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
