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

use candle_core::Device;
use language_graph_engine::hdc_engine::{
    build_scene, build_query, resolve_query, parse_sentence_to_scene, resolve_time_query, HdcBridge, WorkingMemory,
};

#[test]
fn test_sqlite_bridge() {
    let device = Device::Cpu;
    let mut bridge = HdcBridge::new(device).unwrap();
    bridge.init_mock_db().unwrap();

    // Check vocabulary mapping
    let wolf_cid = bridge.get_word_cid("wolf").unwrap();
    assert_eq!(wolf_cid, "urn:language-graph:written-form:wolf");

    let grandmother_cid = bridge.get_word_cid("grandmother").unwrap();
    assert_eq!(grandmother_cid, "urn:language-graph:written-form:grandmother");

    // Check vectors were generated
    let wolf_vec = bridge.vocab_vectors.get(&wolf_cid).unwrap();
    assert_eq!(wolf_vec.dim(0).unwrap(), 4096);
    
    // Ensure determinism
    let mut bridge2 = HdcBridge::new(Device::Cpu).unwrap();
    bridge2.init_mock_db().unwrap();
    
    let wolf_vec2 = bridge2.vocab_vectors.get(&wolf_cid).unwrap();
    // They should be identical
    let diff = wolf_vec.broadcast_sub(wolf_vec2).unwrap().abs().unwrap().sum_all().unwrap().to_scalar::<f32>().unwrap();
    assert_eq!(diff, 0.0);
}

#[test]
fn test_working_memory() {
    let device = Device::Cpu;
    let mut bridge = HdcBridge::new(device.clone()).unwrap();
    bridge.init_mock_db().unwrap();

    let mut wm = WorkingMemory::new(device);
    
    // Scene 1: Wolf ate grandmother
    let wolf_cid = bridge.get_word_cid("wolf").unwrap();
    let ate_cid = bridge.get_word_cid("ate").unwrap();
    let grandmother_cid = bridge.get_word_cid("grandmother").unwrap();

    let scene1 = build_scene(&bridge, &wolf_cid, &ate_cid, &grandmother_cid).unwrap();
    wm.append_scene(&scene1).unwrap();

    let mem = wm.memory_matrix.as_ref().unwrap();
    assert_eq!(mem.dims(), &[1, 4096]);

    // Scene 2: Grandmother ate the bed (nonsense but valid syntactically)
    let bed_cid = bridge.get_word_cid("bed").unwrap();
    let scene2 = build_scene(&bridge, &grandmother_cid, &ate_cid, &bed_cid).unwrap();
    wm.append_scene(&scene2).unwrap();

    let mem = wm.memory_matrix.as_ref().unwrap();
    assert_eq!(mem.dims(), &[2, 4096]);
}

#[test]
fn test_query_resolution() {
    let device = Device::Cpu;
    let mut bridge = HdcBridge::new(device.clone()).unwrap();
    bridge.init_mock_db().unwrap();

    let mut wm = WorkingMemory::new(device);
    
    let wolf_cid = bridge.get_word_cid("wolf").unwrap();
    let ate_cid = bridge.get_word_cid("ate").unwrap();
    let grandmother_cid = bridge.get_word_cid("grandmother").unwrap();

    // Scene: Wolf ate grandmother
    let scene = build_scene(&bridge, &wolf_cid, &ate_cid, &grandmother_cid).unwrap();
    wm.append_scene(&scene).unwrap();

    // Query: who ate grandmother? (Missing subject)
    // Query = Verb * ate + Object * grandmother
    let query = build_query(&bridge, &ate_cid, &grandmother_cid).unwrap();

    // Resolve query targeting "subject" role
    let retrieved_cid = resolve_query(&query, "subject", &wm, &bridge).unwrap();
    
    assert_eq!(retrieved_cid, wolf_cid);
}

#[test]
fn test_sequential_ingestion() {
    let device = Device::Cpu;
    let mut bridge = HdcBridge::new(device.clone()).unwrap();
    bridge.init_mock_db().unwrap();

    let mut wm = WorkingMemory::new(device);
    
    let sentences = [
        "The wolf ate the grandmother",
        "The grandmother ate the bed",
        "The wolf ate the bed",
    ];

    for (i, sentence) in sentences.iter().enumerate() {
        let t_step = i + 1;
        let scene = parse_sentence_to_scene(sentence, &bridge).unwrap();
        wm.append_scene_at_time(&scene, t_step, &bridge).unwrap();
    }

    let mem = wm.memory_matrix.as_ref().unwrap();
    assert_eq!(mem.dims(), &[3, 4096]);
}

#[test]
fn test_time_traversal() {
    let device = Device::Cpu;
    let mut bridge = HdcBridge::new(device.clone()).unwrap();
    bridge.init_mock_db().unwrap();

    let mut wm = WorkingMemory::new(device);
    
    let sentences = [
        "The wolf ate the grandmother",
        "The grandmother ate the bed",
        "The wolf ate the bed",
    ];

    for (i, sentence) in sentences.iter().enumerate() {
        let t_step = i + 1;
        let scene = parse_sentence_to_scene(sentence, &bridge).unwrap();
        wm.append_scene_at_time(&scene, t_step, &bridge).unwrap();
    }

    // Query: What was the subject at T_2?
    let target_role = "subject";
    let retrieved_cid = resolve_time_query(2, target_role, &wm, &bridge).unwrap();
    
    let grandmother_cid = bridge.get_word_cid("grandmother").unwrap();
    assert_eq!(retrieved_cid, grandmother_cid);
}
