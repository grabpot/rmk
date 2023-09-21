use crate::{action::KeyAction, matrix::KeyState};
use log::warn;

/// KeyMap represents the stack of layers.
/// The conception of KeyMap in rmk is borrowed from qmk: https://docs.qmk.fm/#/keymap.
/// Keymap should be bind to the actual pcb matrix definition by KeyPos.
/// RMK detects hardware key strokes, uses KeyPos to retrieve the action from KeyMap.
pub struct KeyMap<const ROW: usize, const COL: usize, const NUM_LAYER: usize> {
    /// Layers
    layers: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    /// Current state of each layer
    layer_state: [bool; NUM_LAYER],
    /// Default layer number, max: 32
    default_layer: u8,
    /// Layer cache
    layer_cache: [[u8; COL]; ROW],
}

impl<const ROW: usize, const COL: usize, const NUM_LAYER: usize> KeyMap<ROW, COL, NUM_LAYER> {
    /// Initialize a keymap from a matrix of actions
    pub fn new(action_map: [[[KeyAction; COL]; ROW]; NUM_LAYER]) -> KeyMap<ROW, COL, NUM_LAYER> {
        KeyMap {
            layers: action_map,
            layer_state: [true; NUM_LAYER],
            default_layer: 0,
            layer_cache: [[0; COL]; ROW],
        }
    }

    /// Fetch the action in keymap
    /// FIXME: When the layer is changed, release event should be processed in the original layer(layer cache)
    /// See https://github.com/qmk/qmk_firmware/blob/master/quantum/action_layer.c#L299
    pub fn get_action(&mut self, row: usize, col: usize, key_state: KeyState) -> KeyAction {
        if key_state.pressed {
            // If the key is already pressed, check layer cache
            let layer = self.get_layer_from_cache(row, col);
            return self.layers[layer as usize][row][col];
        } else {
            // Iterate from higher layer to lower layer
            for (layer_idx, layer) in self.layers.iter().rev().enumerate() {
                if self.layer_state[layer_idx] {
                    // This layer is activated
                    let action = layer[row][col];
                    if action == KeyAction::Transparent || action == KeyAction::No {
                        continue;
                    }
                    // Cache the layer
                    self.save_layer_cache(row, col, layer_idx as u8);

                    return action;
                }
            }
        }

        KeyAction::No
    }

    fn get_layer_from_cache(&self, row: usize, col: usize) -> u8 {
        self.layer_cache[row][col]
    }

    fn save_layer_cache(&mut self, row: usize, col: usize, layer_num: u8) {
        self.layer_cache[row][col] = layer_num;
    }

    /// Activate given layer
    pub fn activate_layer(&mut self, layer_num: u8) {
        if layer_num as usize >= NUM_LAYER {
            warn!("Not a valid layer {layer_num}, keyboard supports only {NUM_LAYER} layers");
            return;
        }
        self.layer_state[layer_num as usize] = true;
    }

    /// Deactivate given layer
    pub fn deactivate_layer(&mut self, layer_num: u8) {
        if layer_num as usize >= NUM_LAYER {
            warn!("Not a valid layer {layer_num}, keyboard supports only {NUM_LAYER} layers");
            return;
        }
        self.layer_state[layer_num as usize] = false;
    }
}