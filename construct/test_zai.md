# Testing Zai Integration

## Quick Verification Steps

### 1. Check Build
```bash
cd construct
cargo build
```
Expected: "Finished" with no errors

### 2. Verify Configuration
```bash
# Check if config has zai
grep -A3 "zai:" config_example.yaml
```
Expected: Should show zai configuration example

### 3. Test Environment Variable
```bash
# Set a test key (use your real key for actual testing)
export ZAI_API_KEY="test-key-for-verification"

# Verify it's set
echo $ZAI_API_KEY
```

### 4. Run the Bot
```bash
cargo run
```
Expected: Bot starts without errors

### 5. In Matrix Room
```
.status        # Should show bot is running
.agents        # Should list "zai" as available agent
.agent zai     # Should switch to zai agent
.ask Hello     # Should get response from Zai
```

## Expected Output

When using `.agent zai`, you should see:
```
✓ Switched to agent: zai
Model: glm-4.7
Provider: zai
```

When using `.ask Hello`, you should get a response from Zai's GLM-4.7 model.

## Troubleshooting

If you see "Missing ZAI_API_KEY":
```bash
export ZAI_API_KEY="your-actual-api-key"
cargo run
```

If you see "Unsupported provider":
Check that `Cargo.toml` has the correct rig dependency:
```toml
rig-core = { git = "https://github.com/Gibbz/rig.git", branch = "feature/zai-provider" }
```

## Success Indicators

✅ Project builds without errors
✅ Config includes zai example
✅ Bot starts and connects to Matrix
✅ `.agents` command shows zai
✅ `.agent zai` switches successfully
✅ `.ask` commands work with zai

All checks passed? Zai integration is working! 🎉
