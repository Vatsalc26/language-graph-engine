/*
 * Copyright 2026 The Authors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use candle_core::{Device, Tensor};
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Element-wise multiplication to bind two concepts.
pub fn bind(t1: &Tensor, t2: &Tensor) -> candle_core::Result<Tensor> {
    t1.broadcast_mul(t2)
}

/// Element-wise addition, immediately followed by squashing back to +1.0 and -1.0.
pub fn bundle(t1: &Tensor, t2: &Tensor) -> candle_core::Result<Tensor> {
    let sum = t1.broadcast_add(t2)?;
    // Squash: >= 0 becomes +1.0, < 0 becomes -1.0.
    // sum.ge(&0.0) creates a u8 tensor of 0s and 1s.
    let zeros = sum.zeros_like()?;
    let mask = sum.ge(&zeros)?;
    let mask_f32 = mask.to_dtype(candle_core::DType::F32)?;
    
    // (mask * 2.0) - 1.0
    let two = Tensor::new(2.0f32, sum.device())?;
    let one = Tensor::new(1.0f32, sum.device())?;
    
    let scaled = mask_f32.broadcast_mul(&two)?;
    scaled.broadcast_sub(&one)
}

/// Cyclical roll/shift of the 1D tensor array to encode sequential order.
pub fn shift(t: &Tensor, positions: isize) -> candle_core::Result<Tensor> {
    let t_len = t.dim(0)? as isize;
    let pos = positions.rem_euclid(t_len);
    if pos == 0 {
        return Ok(t.clone());
    }
    
    let pos_usize = pos as usize;
    let t_len_usize = t_len as usize;
    
    let left = t.narrow(0, t_len_usize - pos_usize, pos_usize)?;
    let right = t.narrow(0, 0, t_len_usize - pos_usize)?;
    
    Tensor::cat(&[&left, &right], 0)
}

/// Generates a random Bipolar tensor array matrix using a deterministic seed.
pub fn generate_bipolar_matrix(rows: usize, cols: usize, seed: u64, device: &Device) -> candle_core::Result<Tensor> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut data = Vec::with_capacity(rows * cols);
    for _ in 0..(rows * cols) {
        if rng.random_bool(0.5) {
            data.push(1.0f32);
        } else {
            data.push(-1.0f32);
        }
    }
    Tensor::from_vec(data, (rows, cols), device)
}

/// Calculates Cosine Similarity between a 1D tensor `t` and a 2D `codebook`.
pub fn cosine_similarity_matrix(t: &Tensor, codebook: &Tensor) -> candle_core::Result<Tensor> {
    // t is shape (D,), codebook is shape (N, D)
    // dot product: (N, D) @ (D, 1) -> (N, 1)
    // For bipolar vectors of dimension D, length is sqrt(D).
    // So cos_sim = dot / D
    
    let d = t.dim(0)? as f32;
    let d_tensor = Tensor::new(d, t.device())?;
    
    // Reshape t to (D, 1)
    let t_col = t.unsqueeze(1)?;
    // matmul(codebook, t_col) -> (N, 1)
    let dot = codebook.matmul(&t_col)?;
    
    // Normalize
    dot.broadcast_div(&d_tensor)?.squeeze(1)
}
