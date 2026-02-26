import sys

with open('src/mcp.rs', 'r') as f:
    mcp_content = f.read()

# Fix YAML parse_existing
old_yaml_parse = """    fn parse_existing(&self, content: &str) -> Result<HashMap<String, Value>> {
        let parsed: serde_yaml::Value = serde_yaml::from_str(content).context("Failed to parse YAML")?;
        if let Some(key) = self.wrapper_key {
            if let Some(wrapped) = parsed.get(key) {
                if let Some(obj) = wrapped.as_mapping() {
                   let mut res = HashMap::new();
                   for (k, v) in obj {
                       let key_str = k.as_str().unwrap_or_default().to_string();
                       let json_v = serde_json::to_value(v)?;
                       res.insert(key_str, json_v);
                   }
                   return Ok(res);
                }
            }
        }
        Ok(HashMap::new())
    }"""

new_yaml_parse = """    fn parse_existing(&self, content: &str) -> Result<HashMap<String, Value>> {
        let parsed: serde_yaml::Value = serde_yaml::from_str(content).context("Failed to parse YAML")?;
        if let Some(obj) = self.wrapper_key
            .and_then(|key| parsed.get(key))
            .and_then(|v| v.as_mapping())
        {
            let mut res = HashMap::new();
            for (k, v) in obj {
                let key_str = k.as_str().unwrap_or_default().to_string();
                let json_v = serde_json::to_value(v)?;
                res.insert(key_str, json_v);
            }
            return Ok(res);
        }
        Ok(HashMap::new())
    }"""

# Fix Continue parse_existing
old_continue_parse = """    fn parse_existing(&self, content: &str) -> Result<HashMap<String, Value>> {
        let parsed: Value = serde_json::from_str(content).unwrap_or(json!({}));
        if let Some(mcp) = parsed.get("mcpServers") {
            if let Some(obj) = mcp.as_object() {
                return Ok(obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
            }
        }
        Ok(HashMap::new())
    }"""

new_continue_parse = """    fn parse_existing(&self, content: &str) -> Result<HashMap<String, Value>> {
        let parsed: Value = serde_json::from_str(content).unwrap_or(json!({}));
        if let Some(obj) = parsed.get("mcpServers").and_then(|mcp| mcp.as_object()) {
            return Ok(obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
        }
        Ok(HashMap::new())
    }"""

mcp_content = mcp_content.replace(old_yaml_parse, new_yaml_parse).replace(old_continue_parse, new_continue_parse)
with open('src/mcp.rs', 'w') as f:
    f.write(mcp_content)
