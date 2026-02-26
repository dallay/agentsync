import sys

with open('src/fs.rs', 'r') as f:
    fs_content = f.read()

old_fs = """        if let Some(parent_canon) = dst_abs
            .parent()
            .and_then(|p| fs::canonicalize(p).ok())
            .filter(|p| p.starts_with(&src_canon))
        {"""

new_fs = """        if dst_abs
            .parent()
            .and_then(|p| fs::canonicalize(p).ok())
            .filter(|p| p.starts_with(&src_canon))
            .is_some()
        {"""

fs_content = fs_content.replace(old_fs, new_fs)
with open('src/fs.rs', 'w') as f:
    f.write(fs_content)

with open('src/mcp.rs', 'r') as f:
    mcp_content = f.read()

# Fix YAML parse_existing (nested ifs)
old_yaml_parse = """    fn parse_existing(&self, content: &str) -> Result<HashMap<String, Value>> {
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

# Wait, I previously used python script to fix clippy, let me re-read it.
# Actually I'll just write it correctly.
new_yaml_parse = """    fn parse_existing(&self, content: &str) -> Result<HashMap<String, Value>> {
        let parsed: serde_yaml::Value =
            serde_yaml::from_str(content).context("Failed to parse YAML")?;
        if let Some(obj) = self
            .wrapper_key
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

# Actually the error was: this  statement can be collapsed
# but I already used .and_then chain. Clippy might still complain if I have if let inside if let.
# Let's check the code.
