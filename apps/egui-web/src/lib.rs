use eframe::egui;
use std::cell::RefCell;
use std::cmp::Ordering;

const REM: f32 = 32.0;
const STATE_CIRCLE_SIZE: f32 = 1.25 * REM;
const STATE_CIRCLE_GAP: f32 = 0.5 * REM;
const STATE_CIRCLE_BOTTOM_MARGIN: f32 = 2.0 * REM;
const STATE_CIRCLE_STROKE: f32 = 2.0;

const MIN_QUBITS: usize = 2;
const MAX_QUBITS: usize = 16;

const LINE_Y: f32 = 6.5 * REM;
const LINE_GAP: f32 = 1.5 * REM;
const LINE_LEFT_OFFSET: f32 = 2.0 * REM;
const LINE_RIGHT_OFFSET: f32 = 2.0 * REM;

const GATE_SIZE: f32 = 1.0 * REM;
const SLOT_SPACING: f32 = GATE_SIZE * 1.5;
const SNAP_DISTANCE: f32 = 0.5625 * REM;

const PALETTE_SIZE: f32 = GATE_SIZE;
const PALETTE_GAP: f32 = 0.5 * REM;
const PALETTE_ROW_Y: f32 = 2.0 * REM;

thread_local! {
  static STATE_VECTOR: RefCell<Vec<f32>> = RefCell::new(vec![1.0, 0.0, 0.0, 0.0]);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GateKind {
  H,
  X,
  Y,
  Z,
  SqrtX,
  S,
  SDagger,
  T,
  TDagger,
}

impl GateKind {
  fn label(self) -> &'static str {
    match self {
      GateKind::H => "H",
      GateKind::X => "X",
      GateKind::Y => "Y",
      GateKind::Z => "Z",
      GateKind::SqrtX => "√X",
      GateKind::S => "S",
      GateKind::SDagger => "S†",
      GateKind::T => "T",
      GateKind::TDagger => "T†",
    }
  }
}

const PALETTE_GATES: [GateKind; 9] = [
  GateKind::H,
  GateKind::X,
  GateKind::Y,
  GateKind::Z,
  GateKind::SqrtX,
  GateKind::S,
  GateKind::SDagger,
  GateKind::T,
  GateKind::TDagger,
];

#[derive(Clone, Copy, Debug)]
struct Complex {
  re: f32,
  im: f32,
}

impl Complex {
  fn new(re: f32, im: f32) -> Self {
    Self { re, im }
  }

  fn add(self, other: Self) -> Self {
    Self::new(self.re + other.re, self.im + other.im)
  }

  fn mul(self, other: Self) -> Self {
    Self::new(self.re * other.re - self.im * other.im, self.re * other.im + self.im * other.re)
  }

  fn abs2(self) -> f32 {
    self.re * self.re + self.im * self.im
  }

  fn phase(self) -> f32 {
    self.im.atan2(self.re)
  }
}

#[derive(Clone, Debug)]
struct PlacedGate {
  id: u32,
  kind: GateKind,
  pos: egui::Pos2,
  wire: usize,
}

struct DragState {
  id: u32,
  offset: egui::Vec2,
}

#[derive(Clone, Debug)]
struct LayoutMetrics {
  line_left: f32,
  line_right: f32,
  line_ys: Vec<f32>,
  slot_left: f32,
  slot_right: f32,
  slot_centers: Vec<f32>,
}

fn layout_metrics(width: f32, qubit_count: usize) -> LayoutMetrics {
  let line_left = LINE_LEFT_OFFSET;
  let line_right = width - LINE_RIGHT_OFFSET;
  let line_ys = (0..qubit_count)
    .map(|index| LINE_Y + LINE_GAP * index as f32)
    .collect::<Vec<f32>>();
  let slot_left = line_left + GATE_SIZE;
  let slot_right = line_right - GATE_SIZE;
  let slot_count = if SLOT_SPACING > 0.0 {
    ((slot_right - slot_left) / SLOT_SPACING).floor() as i32 + 1
  } else {
    0
  };
  let slot_centers = if slot_count > 0 {
    (0..slot_count)
      .map(|index| slot_left + SLOT_SPACING * index as f32)
      .collect()
  } else {
    Vec::new()
  };
  LayoutMetrics {
    line_left,
    line_right,
    line_ys,
    slot_left,
    slot_right,
    slot_centers,
  }
}

fn nearest_slot_center(x: f32, slot_centers: &[f32]) -> (f32, f32) {
  let mut nearest = x;
  let mut nearest_distance = f32::MAX;
  for &slot in slot_centers {
    let distance = (x - slot).abs();
    if distance < nearest_distance {
      nearest = slot;
      nearest_distance = distance;
    }
  }
  (nearest, nearest_distance)
}

fn nearest_available_slot(x: f32, wire_index: usize, ignore_id: Option<u32>, gates: &[PlacedGate], slot_centers: &[f32]) -> Option<(f32, f32)> {
  let mut occupied = Vec::new();
  for gate in gates {
    if gate.wire != wire_index {
      continue;
    }
    if ignore_id == Some(gate.id) {
      continue;
    }
    let center_x = gate.pos.x + GATE_SIZE / 2.0;
    let (snapped, _) = nearest_slot_center(center_x, slot_centers);
    occupied.push(snapped);
  }

  let mut nearest = x;
  let mut nearest_distance = f32::MAX;
  let mut found = false;
  for &slot in slot_centers {
    if occupied.iter().any(|&value| (value - slot).abs() < f32::EPSILON) {
      continue;
    }
    let distance = (x - slot).abs();
    if !found || distance < nearest_distance {
      nearest = slot;
      nearest_distance = distance;
      found = true;
    }
  }
  if found {
    Some((nearest, nearest_distance))
  } else {
    None
  }
}

fn nearest_line(y: f32, line_ys: &[f32]) -> (f32, f32, usize) {
  let mut nearest = line_ys[0];
  let mut nearest_distance = (y - line_ys[0]).abs();
  let mut nearest_index = 0;
  for (index, &line_y) in line_ys.iter().enumerate() {
    let distance = (y - line_y).abs();
    if distance < nearest_distance {
      nearest = line_y;
      nearest_distance = distance;
      nearest_index = index;
    }
  }
  (nearest, nearest_distance, nearest_index)
}

fn gate_matrix(kind: GateKind) -> [[Complex; 2]; 2] {
  let inv_sqrt2 = std::f32::consts::FRAC_1_SQRT_2;
  match kind {
    GateKind::H => [
      [Complex::new(inv_sqrt2, 0.0), Complex::new(inv_sqrt2, 0.0)],
      [Complex::new(inv_sqrt2, 0.0), Complex::new(-inv_sqrt2, 0.0)],
    ],
    GateKind::X => [
      [Complex::new(0.0, 0.0), Complex::new(1.0, 0.0)],
      [Complex::new(1.0, 0.0), Complex::new(0.0, 0.0)],
    ],
    GateKind::Y => [
      [Complex::new(0.0, 0.0), Complex::new(0.0, -1.0)],
      [Complex::new(0.0, 1.0), Complex::new(0.0, 0.0)],
    ],
    GateKind::Z => [
      [Complex::new(1.0, 0.0), Complex::new(0.0, 0.0)],
      [Complex::new(0.0, 0.0), Complex::new(-1.0, 0.0)],
    ],
    GateKind::SqrtX => [
      [Complex::new(0.5, 0.5), Complex::new(0.5, -0.5)],
      [Complex::new(0.5, -0.5), Complex::new(0.5, 0.5)],
    ],
    GateKind::S => [
      [Complex::new(1.0, 0.0), Complex::new(0.0, 0.0)],
      [Complex::new(0.0, 0.0), Complex::new(0.0, 1.0)],
    ],
    GateKind::SDagger => [
      [Complex::new(1.0, 0.0), Complex::new(0.0, 0.0)],
      [Complex::new(0.0, 0.0), Complex::new(0.0, -1.0)],
    ],
    GateKind::T => [
      [Complex::new(1.0, 0.0), Complex::new(0.0, 0.0)],
      [Complex::new(0.0, 0.0), Complex::new(inv_sqrt2, inv_sqrt2)],
    ],
    GateKind::TDagger => [
      [Complex::new(1.0, 0.0), Complex::new(0.0, 0.0)],
      [Complex::new(0.0, 0.0), Complex::new(inv_sqrt2, -inv_sqrt2)],
    ],
  }
}

fn apply_gate_to_state(state: &mut [Complex], kind: GateKind, target: usize, qubits: usize) {
  if target >= qubits {
    return;
  }
  let m = gate_matrix(kind);
  let m00 = m[0][0];
  let m01 = m[0][1];
  let m10 = m[1][0];
  let m11 = m[1][1];
  let bit = qubits.saturating_sub(1).saturating_sub(target);
  let mask = 1usize << bit;

  for index in 0..state.len() {
    if index & mask != 0 {
      continue;
    }
    let pair = index | mask;
    let a0 = state[index];
    let a1 = state[pair];
    state[index] = m00.mul(a0).add(m01.mul(a1));
    state[pair] = m10.mul(a0).add(m11.mul(a1));
  }
}

fn display_index_to_state_index(mut display_index: usize, qubits: usize) -> usize {
  let mut value = 0usize;
  for _ in 0..qubits {
    value = (value << 1) | (display_index & 1);
    display_index >>= 1;
  }
  value
}

fn amplitude_qubits(len: usize) -> usize {
  let mut qubits = 0;
  let mut size = 1usize;
  if len == 0 {
    return 1;
  }
  while size < len {
    size <<= 1;
    qubits += 1;
  }
  qubits.max(1)
}

fn color_rgba(r: f32, g: f32, b: f32, a: f32) -> egui::Color32 {
  egui::Color32::from_rgba_unmultiplied(
    (r * 255.0).round() as u8,
    (g * 255.0).round() as u8,
    (b * 255.0).round() as u8,
    (a * 255.0).round() as u8,
  )
}

struct QniApp {
  next_gate_id: u32,
  placed_gates: Vec<PlacedGate>,
  dragging: Option<DragState>,
  hovered_gate_id: Option<u32>,
  hovered_palette_index: Option<usize>,
  qubit_count: usize,
  state_vector: Vec<Complex>,
  needs_recompute: bool,
}

impl QniApp {
  pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
    cc.egui_ctx.set_visuals(egui::Visuals::light());
    let mut app = Self {
      next_gate_id: 1,
      placed_gates: Vec::new(),
      dragging: None,
      hovered_gate_id: None,
      hovered_palette_index: None,
      qubit_count: MIN_QUBITS,
      state_vector: Vec::new(),
      needs_recompute: true,
    };
    app.sync_state_vector();
    app
  }

  fn layout_qubits(&self) -> usize {
    let mut count = self.qubit_count.clamp(MIN_QUBITS, MAX_QUBITS);
    if self.dragging.is_some() && count < MAX_QUBITS {
      count += 1;
    }
    count
  }

  fn state_qubits(&self) -> usize {
    let mut max_wire: Option<usize> = None;
    for gate in &self.placed_gates {
      max_wire = Some(match max_wire {
        Some(current) => current.max(gate.wire),
        None => gate.wire,
      });
    }
    let count = max_wire.map_or(1, |wire| wire + 1);
    count.clamp(1, MAX_QUBITS)
  }

  fn update_qubit_count(&mut self) {
    let mut max_wire = MIN_QUBITS - 1;
    for gate in &self.placed_gates {
      max_wire = max_wire.max(gate.wire);
    }
    self.qubit_count = (max_wire + 1).clamp(MIN_QUBITS, MAX_QUBITS);
  }

  fn sync_state_vector(&mut self) {
    if !self.needs_recompute {
      return;
    }
    let qubits = self.state_qubits();
    let total = 1usize << qubits;
    let mut state = vec![Complex::new(0.0, 0.0); total];
    state[0] = Complex::new(1.0, 0.0);

    let mut gates: Vec<&PlacedGate> = self.placed_gates.iter().collect();
    gates.sort_by(|a, b| {
      a.pos
        .x
        .partial_cmp(&b.pos.x)
        .unwrap_or(Ordering::Equal)
        .then_with(|| a.id.cmp(&b.id))
    });

    for gate in gates {
      apply_gate_to_state(&mut state, gate.kind, gate.wire, qubits);
    }

    self.state_vector = state;
    let mut flat = Vec::with_capacity(total * 2);
    for amp in &self.state_vector {
      flat.push(amp.re);
      flat.push(amp.im);
    }
    STATE_VECTOR.with(|data| {
      data.borrow_mut().clear();
      data.borrow_mut().extend_from_slice(&flat);
    });
    self.needs_recompute = false;
  }

  fn handle_input(
    &mut self,
    content_rect: egui::Rect,
    ctx: &egui::Context,
    screen_rect: egui::Rect,
  ) {
    let pointer = ctx.input(|input| input.pointer.clone());
    let pos = pointer.latest_pos();
    let local_pos = pos.map(|p| egui::pos2(p.x - content_rect.min.x, p.y - content_rect.min.y));
    let palette_width = PALETTE_GATES.len() as f32 * PALETTE_SIZE + (PALETTE_GATES.len() as f32 - 1.0) * PALETTE_GAP;
    let palette_start_x = screen_rect.width() / 2.0 - palette_width / 2.0;
    let palette_rect = egui::Rect::from_min_size(
      egui::pos2(screen_rect.min.x + palette_start_x, screen_rect.min.y + PALETTE_ROW_Y),
      egui::vec2(palette_width, PALETTE_SIZE),
    );
    let metrics = layout_metrics(content_rect.width(), self.layout_qubits());

    if pointer.primary_pressed() {
      if let Some(cursor) = local_pos {
        if let Some((gate_id, offset)) = self
          .placed_gates
          .iter()
          .rev()
          .find(|gate| {
            let gate_rect = egui::Rect::from_min_size(gate.pos, egui::vec2(GATE_SIZE, GATE_SIZE));
            gate_rect.contains(cursor)
          })
          .map(|gate| (gate.id, cursor - gate.pos))
        {
          self.dragging = Some(DragState { id: gate_id, offset });
          self.hovered_gate_id = None;
          self.hovered_palette_index = None;
          return;
        }

        if let Some(cursor_screen) = pos {
          if palette_rect.contains(cursor_screen) {
            let local_x = cursor_screen.x - (screen_rect.min.x + palette_start_x);
          let index = (local_x / (PALETTE_SIZE + PALETTE_GAP)).floor() as i32;
          if index >= 0 && (index as usize) < PALETTE_GATES.len() {
            let in_box = local_x - index as f32 * (PALETTE_SIZE + PALETTE_GAP) <= PALETTE_SIZE;
            if in_box {
              let new_id = self.next_gate_id;
              let new_gate = PlacedGate {
                id: new_id,
                kind: PALETTE_GATES[index as usize],
                pos: egui::pos2(cursor.x - GATE_SIZE / 2.0, cursor.y - GATE_SIZE / 2.0),
                wire: 0,
              };
              self.next_gate_id += 1;
              self.placed_gates.push(new_gate);
              self.dragging = Some(DragState {
                id: new_id,
                offset: egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0),
              });
              self.hovered_palette_index = None;
              self.hovered_gate_id = None;
            }
          }
        }
      }
      }
    }

    if let Some(drag) = self.dragging.as_ref() {
      if pointer.primary_down() {
      if let Some(cursor) = local_pos {
          if let Some(index) = self.placed_gates.iter().position(|gate| gate.id == drag.id) {
            let mut next_pos = cursor - drag.offset;
            let mut next_wire = self.placed_gates[index].wire;
            let center_y = next_pos.y + GATE_SIZE / 2.0;
            let (line_y, distance, line_index) = nearest_line(center_y, &metrics.line_ys);
            if distance <= SNAP_DISTANCE {
              next_pos.y = line_y - GATE_SIZE / 2.0;
              next_wire = line_index;
              let center_x = next_pos.x + GATE_SIZE / 2.0;
              if let Some((slot_center, _)) =
                nearest_available_slot(center_x, line_index, Some(drag.id), &self.placed_gates, &metrics.slot_centers)
              {
                next_pos.x = slot_center - GATE_SIZE / 2.0;
              }
            }
            let gate = &mut self.placed_gates[index];
            gate.pos = next_pos;
            gate.wire = next_wire;
            ctx.request_repaint();
          }
        }
      }
    } else if let Some(cursor) = local_pos {
      let mut hovered_gate = None;
      for gate in &self.placed_gates {
        let gate_rect = egui::Rect::from_min_size(gate.pos, egui::vec2(GATE_SIZE, GATE_SIZE));
        if gate_rect.contains(cursor) {
          hovered_gate = Some(gate.id);
          break;
        }
      }
      self.hovered_gate_id = hovered_gate;

      let mut hovered_palette = None;
      if let Some(cursor_screen) = pos {
        if palette_rect.contains(cursor_screen) {
          let local_x = cursor_screen.x - (screen_rect.min.x + palette_start_x);
          let index = (local_x / (PALETTE_SIZE + PALETTE_GAP)).floor() as i32;
          if index >= 0 && (index as usize) < PALETTE_GATES.len() {
            let in_box = local_x - index as f32 * (PALETTE_SIZE + PALETTE_GAP) <= PALETTE_SIZE;
            if in_box {
              hovered_palette = Some(index as usize);
            }
          }
        }
      }
      self.hovered_palette_index = hovered_palette;
    } else {
      self.hovered_gate_id = None;
      self.hovered_palette_index = None;
    }

    if pointer.primary_released() {
      if let Some(drag) = self.dragging.take() {
        if let Some(index) = self.placed_gates.iter().position(|gate| gate.id == drag.id) {
          let gate_pos = self.placed_gates[index].pos;
          let gate_id = self.placed_gates[index].id;
          let center_x = gate_pos.x + GATE_SIZE / 2.0;
          let center_y = gate_pos.y + GATE_SIZE / 2.0;
          let (line_y, distance, line_index) = nearest_line(center_y, &metrics.line_ys);
          let snapped =
            nearest_available_slot(center_x, line_index, Some(gate_id), &self.placed_gates, &metrics.slot_centers);
          let on_circuit =
            center_x >= metrics.slot_left
              && center_x <= metrics.slot_right
              && distance <= SNAP_DISTANCE
              && snapped.map(|(_, d)| d <= SNAP_DISTANCE).unwrap_or(false);

          if !on_circuit {
            self.placed_gates.remove(index);
          } else if let Some((slot_center, _)) = snapped {
            let gate = &mut self.placed_gates[index];
            gate.pos.x = slot_center - GATE_SIZE / 2.0;
            gate.pos.y = line_y - GATE_SIZE / 2.0;
            gate.wire = line_index;
          }
          self.update_qubit_count();
          self.needs_recompute = true;
        }
      }
    }
  }

  fn circuit_content_height(&self, qubit_count: usize, screen_height: f32) -> f32 {
    let line_count = qubit_count.max(1);
    let last_line_y = LINE_Y + LINE_GAP * (line_count.saturating_sub(1)) as f32;
    let content_height = last_line_y + GATE_SIZE + 4.0 * REM;
    content_height.max(screen_height)
  }

  fn draw_circuit(
    &self,
    painter: &egui::Painter,
    rect: egui::Rect,
    metrics: &LayoutMetrics,
    colors: &Colors,
  ) {
    for &line_y in &metrics.line_ys {
      let start = rect.min + egui::vec2(metrics.line_left, line_y);
      let end = rect.min + egui::vec2(metrics.line_right, line_y);
      painter.line_segment([start, end], egui::Stroke::new(2.0, colors.line));
    }

    for gate in &self.placed_gates {
      let gate_rect = egui::Rect::from_min_size(rect.min + gate.pos.to_vec2(), egui::vec2(GATE_SIZE, GATE_SIZE));
      if self.hovered_gate_id == Some(gate.id) {
        let hover_outer = gate_rect.expand(4.0);
        let hover_inner = gate_rect.expand(2.0);
        painter.rect_filled(hover_outer, egui::CornerRadius::same(10), colors.box_border);
        painter.rect_filled(hover_inner, egui::CornerRadius::same(8), colors.background);
      }
      painter.rect_filled(gate_rect, egui::CornerRadius::same(6), colors.box_fill);
      painter.text(
        gate_rect.center(),
        egui::Align2::CENTER_CENTER,
        gate.kind.label(),
        egui::FontId::proportional(18.0),
        colors.label,
      );
    }

    for (index, &line_y) in metrics.line_ys.iter().enumerate() {
      let label_pos = rect.min + egui::vec2(metrics.line_left - 3.0 * 14.0 - 12.0, line_y - 7.0);
      painter.text(
        label_pos,
        egui::Align2::LEFT_TOP,
        format!("q{index}:"),
        egui::FontId::proportional(14.0),
        colors.text,
      );
    }
  }

  fn draw_palette(&self, painter: &egui::Painter, rect: egui::Rect, colors: &Colors) {
    let palette_width = PALETTE_GATES.len() as f32 * PALETTE_SIZE + (PALETTE_GATES.len() as f32 - 1.0) * PALETTE_GAP;
    let palette_start_x = rect.width() / 2.0 - palette_width / 2.0;
    let palette_padding = 1.0 * REM;
    let palette_rect = egui::Rect::from_min_size(
      rect.min + egui::vec2(palette_start_x - palette_padding, PALETTE_ROW_Y - palette_padding),
      egui::vec2(palette_width + palette_padding * 2.0, PALETTE_SIZE + palette_padding * 2.0),
    );
    let palette_corner = egui::CornerRadius::same(14);
    let shadow = egui::epaint::Shadow {
      offset: [0, 6],
      blur: 16,
      spread: 0,
      color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 25),
    };
    painter.add(egui::Shape::Rect(shadow.as_shape(palette_rect, palette_corner)));
    painter.rect_filled(palette_rect, palette_corner, colors.surface);

    for (index, gate) in PALETTE_GATES.iter().enumerate() {
      let gate_x = palette_start_x + index as f32 * (PALETTE_SIZE + PALETTE_GAP);
      let gate_rect = egui::Rect::from_min_size(
        rect.min + egui::vec2(gate_x, PALETTE_ROW_Y),
        egui::vec2(PALETTE_SIZE, PALETTE_SIZE),
      );
      if self.hovered_palette_index == Some(index) {
        let hover_outer = gate_rect.expand(4.0);
        let hover_inner = gate_rect.expand(2.0);
        painter.rect_filled(hover_outer, egui::CornerRadius::same(10), colors.box_border);
        painter.rect_filled(hover_inner, egui::CornerRadius::same(8), colors.background);
      }
      painter.rect_filled(gate_rect, egui::CornerRadius::same(6), colors.box_fill);
      painter.text(
        gate_rect.center(),
        egui::Align2::CENTER_CENTER,
        gate.label(),
        egui::FontId::proportional(18.0),
        colors.label,
      );
    }
  }

  fn draw_state_vector(&self, painter: &egui::Painter, rect: egui::Rect, colors: &Colors) {
    let state_count = self.state_vector.len().max(1);
    let qubits = amplitude_qubits(state_count);
    let gap_ratio = STATE_CIRCLE_GAP / STATE_CIRCLE_SIZE;
    let state_padding = (1.0 * REM).min(rect.width() * 0.05).min(rect.height() * 0.05);
    let top_limit = rect.min.y + PALETTE_ROW_Y + PALETTE_SIZE + 2.0 * REM;
    let mut available_width = rect.width() - state_padding * 2.0;
    let mut available_height = rect.max.y - STATE_CIRCLE_BOTTOM_MARGIN - top_limit;
    if available_width <= 0.0 {
      available_width = rect.width().max(1.0);
    }
    if available_height <= 0.0 {
      available_height = (rect.height() - STATE_CIRCLE_BOTTOM_MARGIN).max(1.0);
    }
    let max_fraction = if state_count <= 4 {
      0.4
    } else if state_count <= 16 {
      0.3
    } else {
      0.25
    };
    let max_height = rect.height() * max_fraction;
    if available_height > max_height {
      available_height = max_height.max(1.0);
    }

    let aspect = (available_width / available_height).max(0.1);
    let mut columns = 1usize;
    let mut rows = state_count;
    let mut best_size = 0.0;
    let mut best_score = f32::INFINITY;
    for candidate in 1..=state_count {
      if state_count % candidate != 0 {
        continue;
      }
      let candidate_rows = state_count / candidate;
      let size_w = available_width / (candidate as f32 + (candidate - 1) as f32 * gap_ratio);
      let size_h = available_height / (candidate_rows as f32 + (candidate_rows - 1) as f32 * gap_ratio);
      let size = size_w.min(size_h).min(STATE_CIRCLE_SIZE).max(0.5);
      let ratio = candidate as f32 / candidate_rows as f32;
      let score = (ratio - aspect).abs();
      if size > best_size + 0.01 || ((size - best_size).abs() <= 0.01 && score < best_score) {
        columns = candidate;
        rows = candidate_rows;
        best_size = size;
        best_score = score;
      }
    }
    let size = best_size.max(0.5);
    let gap = size * gap_ratio;
    let total_width = size * columns as f32 + gap * (columns.saturating_sub(1)) as f32;
    let total_height = size * rows as f32 + gap * (rows.saturating_sub(1)) as f32;
    let base_x = rect.width() / 2.0 - total_width / 2.0;
    let base_y = rect.height() - STATE_CIRCLE_BOTTOM_MARGIN - total_height;
    let radius = size * 0.5;
    let stroke = STATE_CIRCLE_STROKE.min(size * 0.25).max(0.5);
    let scale = size / STATE_CIRCLE_SIZE;
    let inner_radius = (radius - stroke * 0.5 + 0.5 * scale).max(0.0);

    let state_rect = egui::Rect::from_min_size(
      rect.min + egui::vec2(base_x - state_padding, base_y - state_padding),
      egui::vec2(total_width + state_padding * 2.0, total_height + state_padding * 2.0),
    );
    let state_corner = egui::CornerRadius::same(14);
    let state_shadow = egui::epaint::Shadow {
      offset: [0, 6],
      blur: 16,
      spread: 0,
      color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 25),
    };
    painter.add(egui::Shape::Rect(state_shadow.as_shape(state_rect, state_corner)));
    painter.rect_filled(state_rect, state_corner, colors.surface);

    for i in 0..state_count {
      let state_index = display_index_to_state_index(i, qubits);
      let amplitude = self.state_vector[state_index];
      let probability = amplitude.abs2().clamp(0.0, 1.0);
      let phase_opt = Some(amplitude.phase());
      let base_fill_radius = (radius - stroke * 0.5 + 1.0 * scale).max(0.0);
      let fill_radius = base_fill_radius * probability.sqrt();
      let row = i / columns;
      let col = i % columns;
      let x = base_x + col as f32 * (size + gap);
      let y = base_y + row as f32 * (size + gap);
      let center = rect.min + egui::vec2(x + radius, y + radius);
      let outline = if probability > 0.0 {
        colors.state_outline
      } else {
        colors.state_outline_zero
      };

      painter.circle_filled(center, inner_radius, colors.surface);
      if fill_radius > 0.0 {
        painter.circle_filled(center, fill_radius, colors.state_fill);
      }
      painter.circle_stroke(center, radius, egui::Stroke::new(stroke, outline));

      if probability > 0.0 {
        let phase = phase_opt.unwrap_or(0.0);
        let dir = egui::vec2(phase.sin(), -phase.cos());
        let needle_radius = inner_radius;
        painter.line_segment(
          [center, center + dir * needle_radius],
          egui::Stroke::new(stroke, colors.state_needle),
        );
      }
    }
  }
}

impl eframe::App for QniApp {
  fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    egui::CentralPanel::default().show(ctx, |ui| {
      let screen_rect = ui.max_rect();
      let colors = Colors::new();
      let content_height = self.circuit_content_height(self.layout_qubits(), screen_rect.height());

      egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .scroll_source(egui::scroll_area::ScrollSource {
          drag: false,
          ..egui::scroll_area::ScrollSource::default()
        })
        .show(ui, |ui| {
          let (rect, _response) =
            ui.allocate_exact_size(egui::vec2(screen_rect.width(), content_height), egui::Sense::click_and_drag());
          self.handle_input(rect, ctx, screen_rect);
          self.sync_state_vector();

          let metrics = layout_metrics(rect.width(), self.layout_qubits());
          let painter = ui.painter_at(rect);
          self.draw_circuit(&painter, rect, &metrics, &colors);
        });

      let overlay_painter =
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("overlay")));
      self.draw_palette(&overlay_painter, screen_rect, &colors);
      self.draw_state_vector(&overlay_painter, screen_rect, &colors);
    });
  }
}

struct Colors {
  background: egui::Color32,
  surface: egui::Color32,
  line: egui::Color32,
  box_fill: egui::Color32,
  box_border: egui::Color32,
  label: egui::Color32,
  text: egui::Color32,
  state_fill: egui::Color32,
  state_outline: egui::Color32,
  state_outline_zero: egui::Color32,
  state_needle: egui::Color32,
}

impl Colors {
  fn new() -> Self {
    Self {
      background: color_rgba(0.976, 0.98, 0.984, 1.0),
      surface: color_rgba(1.0, 1.0, 1.0, 1.0),
      line: color_rgba(0.72, 0.72, 0.72, 1.0),
      box_fill: color_rgba(0.2, 0.62, 0.55, 1.0),
      box_border: color_rgba(0.82, 0.82, 0.82, 1.0),
      label: color_rgba(1.0, 1.0, 1.0, 1.0),
      text: color_rgba(0.45, 0.45, 0.45, 1.0),
      state_fill: color_rgba(0.16, 0.58, 0.78, 1.0),
      state_outline: color_rgba(0.0, 0.0, 0.0, 1.0),
      state_outline_zero: color_rgba(0.75, 0.75, 0.75, 1.0),
      state_needle: color_rgba(0.0, 0.0, 0.0, 1.0),
    }
  }
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn start(canvas_id: &str) -> Result<(), wasm_bindgen::JsValue> {
  let window = web_sys::window().ok_or_else(|| wasm_bindgen::JsValue::from_str("window not found"))?;
  let document = window
    .document()
    .ok_or_else(|| wasm_bindgen::JsValue::from_str("document not found"))?;
  let canvas = document
    .get_element_by_id(canvas_id)
    .ok_or_else(|| wasm_bindgen::JsValue::from_str("canvas not found"))?
    .dyn_into::<web_sys::HtmlCanvasElement>()?;

  let web_options = eframe::WebOptions::default();
  eframe::WebRunner::new()
    .start(
      canvas,
      web_options,
      Box::new(|cc| Ok(Box::new(QniApp::new(cc)))),
    )
    .await
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn read_state_vector() -> js_sys::Float32Array {
  STATE_VECTOR.with(|data| {
    let data = data.borrow();
    let output = js_sys::Float32Array::new_with_length(data.len() as u32);
    output.copy_from(data.as_slice());
    output
  })
}
