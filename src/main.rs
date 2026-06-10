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

mod hdc;

use candle_core::{Device, Tensor};
use std::collections::HashMap;

fn main() -> candle_core::Result<()> {
    println!("=== HDC Spatial Logic Engine Prototype ===");

    let device = Device::Cpu;
    let d = 4096;
    let num_chars = 95;

    println!("\n--- Module 2: Layer 1 Codebook & Bridge ---");
    let codebook = hdc::generate_bipolar_matrix(num_chars, d, 42, &device)?;
    
    let mut char_to_idx = HashMap::new();
    let mut idx_to_cid = HashMap::new();
    
    for i in 0..num_chars {
        let ch = (0x20 + i) as u8 as char;
        char_to_idx.insert(ch, i);
        idx_to_cid.insert(i, format!("urn:language-graph:grapheme:{:02x}", ch as u8));
    }
    println!("Generated 95x4096 codebook and CIDs.");

    println!("\n--- Module 3: Layer 2 Discovery Engine (Context Vectoring) ---");
    let text = "the cat ran in the hat";
    let chars: Vec<char> = text.chars().collect();
    
    let mut context_sums: HashMap<char, Tensor> = HashMap::new();
    let mut context_counts: HashMap<char, usize> = HashMap::new();

    for i in 1..(chars.len() - 1) {
        let left_ch = chars[i - 1];
        let target_ch = chars[i];
        let right_ch = chars[i + 1];

        let left_idx = *char_to_idx.get(&left_ch).unwrap();
        let right_idx = *char_to_idx.get(&right_ch).unwrap();

        let left_vec = codebook.narrow(0, left_idx, 1)?.squeeze(0)?;
        let right_vec = codebook.narrow(0, right_idx, 1)?.squeeze(0)?;

        let left_shifted = hdc::shift(&left_vec, -1)?;
        let right_shifted = hdc::shift(&right_vec, 1)?;

        let ctx_vec = hdc::bundle(&left_shifted, &right_shifted)?;

        if let Some(sum) = context_sums.get(&target_ch) {
            let new_sum = sum.broadcast_add(&ctx_vec)?;
            context_sums.insert(target_ch, new_sum);
            *context_counts.get_mut(&target_ch).unwrap() += 1;
        } else {
            context_sums.insert(target_ch, ctx_vec);
            context_counts.insert(target_ch, 1);
        }
    }

    println!("Cosine Similarity between average context vectors of characters:");
    let mut target_chars: Vec<char> = context_sums.keys().cloned().collect();
    target_chars.sort();

    let mut averaged_contexts = HashMap::new();
    for ch in &target_chars {
        let sum = context_sums.get(ch).unwrap();
        let count = *context_counts.get(ch).unwrap() as f32;
        let count_tensor = Tensor::new(count, &device)?;
        let avg = sum.broadcast_div(&count_tensor)?;
        averaged_contexts.insert(*ch, avg);
    }

    for i in 0..target_chars.len() {
        for j in (i+1)..target_chars.len() {
            let c1 = target_chars[i];
            let c2 = target_chars[j];
            let v1 = averaged_contexts.get(&c1).unwrap();
            let v2 = averaged_contexts.get(&c2).unwrap();
            
            let dot = v1.unsqueeze(0)?.matmul(&v2.unsqueeze(1)?)?.flatten_all()?.to_vec1::<f32>()?[0];
            let norm1 = v1.sqr()?.sum_all()?.flatten_all()?.to_vec1::<f32>()?[0].sqrt();
            let norm2 = v2.sqr()?.sum_all()?.flatten_all()?.to_vec1::<f32>()?[0].sqrt();
            let sim = dot / (norm1 * norm2);
            
            // Only print similarities greater than a threshold to avoid flooding
            if sim > 0.05 { 
                println!("Sim('{}', '{}') = {:.4}", c1, c2, sim);
            }
        }
    }

    println!("\n--- Module 4: The Execution Query (Equation Resolution) ---");
    let role_pos1 = hdc::generate_bipolar_matrix(1, d, 101, &device)?.squeeze(0)?;
    let role_pos2 = hdc::generate_bipolar_matrix(1, d, 102, &device)?.squeeze(0)?;

    let a_idx = *char_to_idx.get(&'a').unwrap();
    let n_idx = *char_to_idx.get(&'n').unwrap();
    let char_a = codebook.narrow(0, a_idx, 1)?.squeeze(0)?;
    let char_n = codebook.narrow(0, n_idx, 1)?.squeeze(0)?;

    let bind_a = hdc::bind(&char_a, &role_pos1)?;
    let bind_n = hdc::bind(&char_n, &role_pos2)?;

    let word_an = hdc::bundle(&bind_a, &bind_n)?;

    println!("Light Up: W = bundle(bind(a, pos1), bind(n, pos2))");
    println!("Clean-Up: Isolate the missing letter 'a' from 'an'");
    
    let remainder = word_an.broadcast_sub(&bind_n)?;
    let target = hdc::bind(&remainder, &role_pos1)?;

    let sims = hdc::cosine_similarity_matrix(&target, &codebook)?;
    let sims_vec = sims.to_vec1::<f32>()?;
    
    let mut best_idx = 0;
    let mut best_score = -2.0;

    for (i, &score) in sims_vec.iter().enumerate() {
        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }

    let resolved_cid = idx_to_cid.get(&best_idx).unwrap();
    let resolved_char = (0x20 + best_idx) as u8 as char;
    
    println!("Resolution instantaneous. Highest match:");
    println!("Score: {:.4}", best_score);
    println!("Char: '{}'", resolved_char);
    println!("CID: {}", resolved_cid);

    assert_eq!(resolved_char, 'a');

    Ok(())
}
