import { struct, int, text } from "./wire-decode";
const u32 = int(4294967295),
  u16 = int(65535);
const bot = struct({
  decision_ms: u32,
  accusation_ms: u32,
  max_nodes: int(),
  max_component_cells: int(),
  max_constraints: int(),
});
const rules = struct({
  version: u16,
  width: int(16, 1),
  height: int(16, 1),
  mines: u16,
  opening: u16,
  duration_ms: u32,
  countdown_ms: u32,
  mine_stun_ms: u32,
  wrong_accuse_stun_ms: u32,
  reflect_stun_ms: u32,
  gauge_capacity: u16,
  safe_gain: u16,
  max_lies: int(255),
  reconnect_grace_ms: u32,
  room_expiry_ms: u32,
  max_commands_per_seat: u32,
  attack_strategy: text(/^[a-z0-9-]{1,64}$/),
  bots: struct({ easy: bot, normal: bot, hard: bot }),
});
export const decodeRulesSnapshot = struct({
  rules,
  hash: text(/^[0-9a-f]{64}$/),
  solver_version: u16,
  rng_version: u16,
});
