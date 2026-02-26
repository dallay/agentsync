
import os

# Create a large AGENTS.md
agents_md_content = "# Project Agents\n\n" + "\n".join([f"## Agent {i}\nThis is agent {i} configuration." for i in range(1000)])
os.makedirs(".agents", exist_ok=True)
with open(".agents/AGENTS.md", "w") as f:
    f.write(agents_md_content)

# Create agentsync.toml with many agents
config = """
source_dir = "."
compress_agents_md = true

[gitignore]
enabled = false
"""

for i in range(100):
    config += f"""
[agents.agent{i}]
enabled = true
[agents.agent{i}.targets.main]
source = "AGENTS.md"
destination = "agent{i}.md"
type = "symlink"
"""

with open(".agents/agentsync.toml", "w") as f:
    f.write(config)
