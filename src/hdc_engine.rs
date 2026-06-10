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

use crate::hdc::{bind, bundle, cosine_similarity_matrix, generate_bipolar_matrix};
use candle_core::{Device, Tensor};
use rusqlite::Connection;
use std::collections::HashMap;
use std::error::Error;

pub struct HdcBridge {
    pub conn: Connection,
    pub vocab_vectors: HashMap<String, Tensor>,
    pub role_vectors: HashMap<String, Tensor>,
    pub device: Device,
}

impl HdcBridge {
    pub fn new(device: Device) -> Result<Self, Box<dyn Error>> {
        let conn = Connection::open_in_memory()?;
        conn.execute(
            "CREATE TABLE vocabulary (
                id INTEGER PRIMARY KEY,
                word TEXT NOT NULL UNIQUE,
                cid TEXT NOT NULL UNIQUE
            )",
            [],
        )?;
        
        Ok(Self {
            conn,
            vocab_vectors: HashMap::new(),
            role_vectors: HashMap::new(),
            device,
        })
    }

    pub fn init_mock_db(&mut self) -> Result<(), Box<dyn Error>> {
        let words = vec!["wolf", "grandmother", "ate", "the", "bed"];
        
        for (i, word) in words.iter().enumerate() {
            let cid = format!("urn:language-graph:written-form:{}", word);
            self.conn.execute(
                "INSERT INTO vocabulary (word, cid) VALUES (?1, ?2)",
                (word, &cid),
            )?;
            
            // Generate a deterministic 4096-D vector for each word.
            let seed = 1000 + i as u64;
            let vector = generate_bipolar_matrix(1, 4096, seed, &self.device)?;
            let vector_1d = vector.squeeze(0)?;
            self.vocab_vectors.insert(cid, vector_1d);
        }

        // Generate Role vectors deterministically.
        let roles = vec!["subject", "verb", "object"];
        for (i, role) in roles.iter().enumerate() {
            let seed = 2000 + i as u64;
            let vector = generate_bipolar_matrix(1, 4096, seed, &self.device)?;
            let vector_1d = vector.squeeze(0)?;
            self.role_vectors.insert(role.to_string(), vector_1d);
        }

        Ok(())
    }

    pub fn get_word_cid(&self, word: &str) -> Option<String> {
        let mut stmt = self.conn.prepare("SELECT cid FROM vocabulary WHERE word = ?1").ok()?;
        let mut rows = stmt.query([word]).ok()?;
        if let Some(row) = rows.next().ok()? {
            let cid: String = row.get(0).ok()?;
            return Some(cid);
        }
        None
    }
}

pub struct WorkingMemory {
    pub memory_matrix: Option<Tensor>,
    pub device: Device,
}

impl WorkingMemory {
    pub fn new(device: Device) -> Self {
        Self {
            memory_matrix: None,
            device,
        }
    }

    /// Appends a new scene vector (1D tensor of shape [4096]) to the memory matrix.
    pub fn append_scene(&mut self, scene_vector: &Tensor) -> Result<(), Box<dyn Error>> {
        let scene_2d = if scene_vector.rank() == 1 {
            scene_vector.unsqueeze(0)?
        } else {
            scene_vector.clone()
        };

        if let Some(ref current_mem) = self.memory_matrix {
            self.memory_matrix = Some(Tensor::cat(&[current_mem, &scene_2d], 0)?);
        } else {
            self.memory_matrix = Some(scene_2d);
        }
        Ok(())
    }
}

pub fn build_scene(
    bridge: &HdcBridge,
    subject_cid: &str,
    verb_cid: &str,
    object_cid: &str,
) -> Result<Tensor, Box<dyn Error>> {
    let s_vec = bridge.vocab_vectors.get(subject_cid).ok_or("CID not found")?;
    let v_vec = bridge.vocab_vectors.get(verb_cid).ok_or("CID not found")?;
    let o_vec = bridge.vocab_vectors.get(object_cid).ok_or("CID not found")?;

    let s_role = bridge.role_vectors.get("subject").ok_or("Role not found")?;
    let v_role = bridge.role_vectors.get("verb").ok_or("Role not found")?;
    let o_role = bridge.role_vectors.get("object").ok_or("Role not found")?;

    // Bind roles to vectors
    let s_bound = bind(s_role, s_vec)?;
    let v_bound = bind(v_role, v_vec)?;
    let o_bound = bind(o_role, o_vec)?;

    // Bundle them together (s_bound + v_bound + o_bound)
    let temp1 = bundle(&s_bound, &v_bound)?;
    let scene = bundle(&temp1, &o_bound)?;

    Ok(scene)
}

pub fn build_query(
    bridge: &HdcBridge,
    verb_cid: &str,
    object_cid: &str,
) -> Result<Tensor, Box<dyn Error>> {
    let v_vec = bridge.vocab_vectors.get(verb_cid).ok_or("CID not found")?;
    let o_vec = bridge.vocab_vectors.get(object_cid).ok_or("CID not found")?;

    let v_role = bridge.role_vectors.get("verb").ok_or("Role not found")?;
    let o_role = bridge.role_vectors.get("object").ok_or("Role not found")?;

    let v_bound = bind(v_role, v_vec)?;
    let o_bound = bind(o_role, o_vec)?;

    let query = bundle(&v_bound, &o_bound)?;
    Ok(query)
}

pub fn resolve_query(
    query: &Tensor,
    target_role: &str,
    working_memory: &WorkingMemory,
    bridge: &HdcBridge,
) -> Result<String, Box<dyn Error>> {
    let mem = working_memory.memory_matrix.as_ref().ok_or("Memory empty")?;
    
    // Broadcast query against WorkingMemory using cosine similarity
    let similarities = cosine_similarity_matrix(query, mem)?;
    
    // similarities is shape (N,)
    let argmax = similarities.argmax(0)?;
    let best_idx: u32 = argmax.to_scalar::<u32>()?;
    
    let best_scene = mem.narrow(0, best_idx as usize, 1)?.squeeze(0)?;
    
    // X = Scene - Query
    let remainder = best_scene.broadcast_sub(query)?;
    
    // Unbind the target role: X_unbound = remainder * Role
    let role_vector = bridge.role_vectors.get(target_role).ok_or("Role not found")?;
    let unbound = bind(&remainder, role_vector)?;
    
    // Convert vocab vectors to a 2D codebook tensor for cosine similarity
    let mut cids = Vec::new();
    let mut vecs = Vec::new();
    for (cid, vec) in &bridge.vocab_vectors {
        cids.push(cid.clone());
        vecs.push(vec.clone());
    }
    
    let codebook = Tensor::stack(&vecs, 0)?;
    
    // Compute cosine similarity of unbound against all vocab vectors
    let vocab_sims = cosine_similarity_matrix(&unbound, &codebook)?;
    let vocab_argmax = vocab_sims.argmax(0)?;
    let best_vocab_idx: u32 = vocab_argmax.to_scalar::<u32>()?;
    
    let best_cid = cids[best_vocab_idx as usize].clone();
    Ok(best_cid)
}
