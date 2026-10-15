use serde::Serialize;
use vanity_core::{
    build_pattern_list, default_keys_per_sec, expected_attempts_any, grind_estimate, pattern_guide,
    Chain, ChainGrinder, PatternRisk, DEPLOY_MODES, MENU_CHAINS,
};
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct ExportOut {
    label: String,
    value: String,
    hint: Option<String>,
}

#[derive(Serialize)]
struct FoundOut {
    address: String,
    exports: Vec<ExportOut>,
}

#[derive(Serialize)]
struct ChunkOut {
    found: bool,
    attempts: u32,
    result: Option<FoundOut>,
}

#[derive(Serialize)]
struct ChainInfoOut {
    id: String,
    name: String,
}

#[derive(Serialize)]
struct DeployModeOut {
    id: String,
    name: String,
    /// True when caller must pass `deployer` + `initCodeHash` (CREATE2).
    needs_factory: bool,
}

fn resolve_chain(
    chain_id: &str,
    deployer: Option<&str>,
    init_code_hash: Option<&str>,
) -> Result<Chain, JsValue> {
    Chain::resolve_chain(chain_id, deployer, init_code_hash)
        .map_err(|e| js_err("INVALID_CHAIN", &e))
}

#[derive(Serialize)]
struct EstimateOut {
    attempts: f64,
    attempts_label: String,
    avg_secs: f64,
    time_label: String,
    difficulty: String,
    difficulty_bars: String,
    risk: String,
    pattern_chars: usize,
    keys_per_sec: f64,
    guide_verdict: String,
    guide_rule: String,
}

#[derive(Serialize)]
struct ValidateOut {
    ok: bool,
    error: Option<String>,
    risk: Option<String>,
    attempts: Option<f64>,
    attempts_label: Option<String>,
    time_label: Option<String>,
}

fn js_err(code: &str, message: &str) -> JsValue {
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"code".into(), &code.into()).ok();
    js_sys::Reflect::set(&obj, &"message".into(), &message.into()).ok();
    obj.into()
}

fn build_patterns(
    chain: &Chain,
    prefix: &str,
    suffix: &str,
    contains: &str,
    ignore_case: bool,
) -> Result<Vec<vanity_core::Pattern>, JsValue> {
    let prefix_opt = if prefix.is_empty() {
        None
    } else {
        Some(prefix)
    };
    let suffix_opt = if suffix.is_empty() {
        None
    } else {
        Some(suffix)
    };
    let contains_opt = if contains.is_empty() {
        None
    } else {
        Some(contains)
    };
    if prefix_opt.is_none() && suffix_opt.is_none() && contains_opt.is_none() {
        return Err(js_err(
            "INVALID_PATTERN",
            "prefix, suffix, or contains is required",
        ));
    }
    let exact = !ignore_case;
    build_pattern_list(chain, prefix_opt, suffix_opt, contains_opt, exact)
        .map_err(|e| js_err("INVALID_PATTERN", &e))
}

fn expected_any(chain: &Chain, patterns: &[vanity_core::Pattern]) -> f64 {
    expected_attempts_any(
        &patterns
            .iter()
            .map(|p| chain.expected_attempts(p))
            .collect::<Vec<_>>(),
    )
}

/// Runs up to `attempts` grind tries. Supports contains + comma OR patterns.
/// For `create2`, pass `deployer` + `init_code_hash` (20- and 32-byte hex).
#[wasm_bindgen]
pub fn grind_chunk(
    chain_id: &str,
    prefix: &str,
    suffix: &str,
    ignore_case: bool,
    attempts: u32,
    contains: Option<String>,
    deployer: Option<String>,
    init_code_hash: Option<String>,
) -> Result<JsValue, JsValue> {
    let chain = resolve_chain(
        chain_id,
        deployer.as_deref(),
        init_code_hash.as_deref(),
    )?;
    let contains = contains.unwrap_or_default();
    let patterns = build_patterns(&chain, prefix, suffix, &contains, ignore_case)?;

    for i in 0..attempts {
        let (address, attempt) = chain.grind_attempt();
        if patterns.iter().any(|p| chain.matches(&address, p)) {
            let kp = chain.finalize(attempt);
            if kp.address != address || !patterns.iter().any(|p| chain.matches(&kp.address, p)) {
                return Err(js_err(
                    "INTERNAL",
                    "derived address did not match the search result; key discarded",
                ));
            }
            let out = ChunkOut {
                found: true,
                attempts: i + 1,
                result: Some(FoundOut {
                    address: kp.address,
                    exports: kp
                        .exports
                        .into_iter()
                        .map(|mut e| ExportOut {
                            label: std::mem::take(&mut e.label),
                            value: std::mem::take(&mut e.value),
                            hint: e.hint.take(),
                        })
                        .collect(),
                }),
            };
            return serde_wasm_bindgen::to_value(&out)
                .map_err(|e| js_err("INTERNAL", &e.to_string()));
        }
    }

    let out = ChunkOut {
        found: false,
        attempts,
        result: None,
    };
    serde_wasm_bindgen::to_value(&out).map_err(|e| js_err("INTERNAL", &e.to_string()))
}

#[wasm_bindgen]
pub fn is_valid_chain(chain_id: &str) -> bool {
    let id = chain_id.trim().to_ascii_lowercase();
    if id == "create2" {
        return true;
    }
    Chain::from_id(chain_id).is_ok()
}

#[wasm_bindgen]
pub fn list_chains() -> Result<JsValue, JsValue> {
    let list: Vec<ChainInfoOut> = MENU_CHAINS
        .iter()
        .map(|(id, name)| ChainInfoOut {
            id: (*id).to_string(),
            name: (*name).to_string(),
        })
        .collect();
    serde_wasm_bindgen::to_value(&list).map_err(|e| js_err("INTERNAL", &e.to_string()))
}

/// Deploy / mint modes (`evm-contract`, `trx-contract`, `sol-mint`, `create2`).
#[wasm_bindgen]
pub fn list_deploy_modes() -> Result<JsValue, JsValue> {
    let list: Vec<DeployModeOut> = DEPLOY_MODES
        .iter()
        .map(|(id, name)| DeployModeOut {
            id: (*id).to_string(),
            name: (*name).to_string(),
            needs_factory: *id == "create2",
        })
        .collect();
    serde_wasm_bindgen::to_value(&list).map_err(|e| js_err("INTERNAL", &e.to_string()))
}

#[derive(Serialize)]
struct ChainDetailOut {
    name: String,
    hint: String,
    sample: String,
}

/// Display name, pattern hint, and one random address (its key is discarded).
/// For `create2`, pass `deployer` + `init_code_hash`.
#[wasm_bindgen]
pub fn chain_details(
    chain_id: &str,
    deployer: Option<String>,
    init_code_hash: Option<String>,
) -> Result<JsValue, JsValue> {
    let chain = resolve_chain(
        chain_id,
        deployer.as_deref(),
        init_code_hash.as_deref(),
    )?;
    let (sample, _discarded) = chain.grind_attempt();
    let out = ChainDetailOut {
        name: chain.display_name().to_string(),
        hint: chain.pattern_hint().to_string(),
        sample,
    };
    serde_wasm_bindgen::to_value(&out).map_err(|e| js_err("INTERNAL", &e.to_string()))
}

#[wasm_bindgen]
pub fn estimate_difficulty(
    chain_id: &str,
    prefix: &str,
    suffix: &str,
    ignore_case: bool,
    keys_per_sec: f64,
    contains: Option<String>,
    deployer: Option<String>,
    init_code_hash: Option<String>,
) -> Result<JsValue, JsValue> {
    let chain = resolve_chain(
        chain_id,
        deployer.as_deref(),
        init_code_hash.as_deref(),
    )?;
    let contains = contains.unwrap_or_default();
    let patterns = build_patterns(&chain, prefix, suffix, &contains, ignore_case)?;
    let attempts = expected_any(&chain, &patterns);
    let kps = if keys_per_sec > 0.0 {
        keys_per_sec
    } else {
        default_keys_per_sec(chain.id())
    };
    let est = grind_estimate(attempts, kps, &patterns[0]);
    let (verdict, rule) = pattern_guide(&est, chain.chars_per_position(ignore_case));
    let out = EstimateOut {
        attempts: est.attempts,
        attempts_label: est.attempts_label,
        avg_secs: est.avg_secs,
        time_label: est.time_label,
        difficulty: est.difficulty.to_string(),
        difficulty_bars: est.difficulty_bars,
        risk: est.risk.as_str().to_string(),
        pattern_chars: est.pattern_chars,
        keys_per_sec: kps,
        guide_verdict: verdict.to_string(),
        guide_rule: rule,
    };
    serde_wasm_bindgen::to_value(&out).map_err(|e| js_err("INTERNAL", &e.to_string()))
}

#[wasm_bindgen]
pub fn validate_pattern(
    chain_id: &str,
    prefix: &str,
    suffix: &str,
    ignore_case: bool,
    contains: Option<String>,
    deployer: Option<String>,
    init_code_hash: Option<String>,
) -> Result<JsValue, JsValue> {
    let chain = match Chain::resolve_chain(
        chain_id,
        deployer.as_deref(),
        init_code_hash.as_deref(),
    ) {
        Ok(c) => c,
        Err(e) => {
            let out = ValidateOut {
                ok: false,
                error: Some(e),
                risk: None,
                attempts: None,
                attempts_label: None,
                time_label: None,
            };
            return serde_wasm_bindgen::to_value(&out)
                .map_err(|err| js_err("INTERNAL", &err.to_string()));
        }
    };

    let contains = contains.unwrap_or_default();
    let patterns = match build_patterns(&chain, prefix, suffix, &contains, ignore_case) {
        Ok(p) => p,
        Err(js) => {
            let message = js_sys::Reflect::get(&js, &"message".into())
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_else(|| "invalid pattern".to_string());
            let out = ValidateOut {
                ok: false,
                error: Some(message),
                risk: None,
                attempts: None,
                attempts_label: None,
                time_label: None,
            };
            return serde_wasm_bindgen::to_value(&out)
                .map_err(|err| js_err("INTERNAL", &err.to_string()));
        }
    };

    let attempts = expected_any(&chain, &patterns);
    let kps = default_keys_per_sec(chain.id());
    let est = grind_estimate(attempts, kps, &patterns[0]);
    let risk = if est.risk == PatternRisk::None {
        None
    } else {
        Some(est.risk.as_str().to_string())
    };
    let out = ValidateOut {
        ok: true,
        error: None,
        risk,
        attempts: Some(est.attempts),
        attempts_label: Some(est.attempts_label),
        time_label: Some(est.time_label),
    };
    serde_wasm_bindgen::to_value(&out).map_err(|e| js_err("INTERNAL", &e.to_string()))
}

#[cfg(test)]
mod tests {
    use vanity_core::{Chain, ChainGrinder, MENU_CHAINS};

    #[test]
    fn finalize_reproduces_the_ground_address_on_every_chain() {
        for (id, _) in MENU_CHAINS {
            let chain = Chain::from_id(id).expect("menu chain");
            for _ in 0..3 {
                let (address, attempt) = chain.grind_attempt();
                assert_eq!(chain.finalize(attempt).address, address, "chain {id}");
            }
        }
    }

    #[test]
    fn evm_grind_attempt_matches_own_pattern() {
        let chain = Chain::from_id("evm").expect("evm chain");
        let pattern = chain
            .build_pattern(Some("a"), None, None, false)
            .expect("build pattern");
        let mut found = false;
        for _ in 0..20_000 {
            let (address, _attempt) = chain.grind_attempt();
            if chain.matches(&address, &pattern) {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "expected a match within 20k attempts for a 1-char prefix"
        );
    }
}
