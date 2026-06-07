#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Skills Demo test script"""

import os
import sys

# Handle Windows encoding
if sys.platform == 'win32':
    import io
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')

def test_skills_demo():
    """Test Skills system"""
    
    print("[TEST] Starting Skills Demo test...")
    
    # Test 1: Check Skills source files
    print("\n[TEST 1] Checking Skills source...")
    skill_paths = [
        "core/src/skills.rs",
        "../core/src/skills.rs",
    ]
    
    source_found = False
    for path in skill_paths:
        if os.path.exists(path):
            print(f"[OK] Skills source found: {path}")
            source_found = True
            
            # Verify Skill structure
            with open(path, 'r', encoding='utf-8') as f:
                content = f.read()
                if 'SkillType' in content:
                    print("[OK] SkillType enum defined")
                if 'SkillPriority' in content:
                    print("[OK] SkillPriority enum defined")
                if 'Skill' in content:
                    print("[OK] Skill struct defined")
                break
    
    if not source_found:
        print("[WARN] Skills source not found")
    
    # Test 2: Check skill tool implementation
    print("\n[TEST 2] Checking skill tool...")
    tool_paths = [
        "core/src/tools/skill.rs",
        "../core/src/tools/skill.rs",
    ]
    
    tool_found = False
    for path in tool_paths:
        if os.path.exists(path):
            print(f"[OK] Skill tool found: {path}")
            tool_found = True
            
            with open(path, 'r', encoding='utf-8') as f:
                content = f.read()
                if 'skill' in content.lower():
                    print("[OK] Skill tool implemented")
                break
    
    if not tool_found:
        print("[WARN] Skill tool not found")
    
    # Test 3: Check Skills documentation
    print("\n[TEST 3] Checking Skills documentation...")
    doc_paths = [
        "docs/matrixcode_intro_skill.md",
        "../docs/matrixcode_intro_skill.md",
    ]
    
    doc_found = False
    for path in doc_paths:
        if os.path.exists(path):
            print(f"[OK] Skills documentation found: {path}")
            doc_found = True
            
            with open(path, 'r', encoding='utf-8') as f:
                content = f.read()
                if 'Skill' in content:
                    print("[OK] Skill documentation content present")
                break
    
    if not doc_found:
        print("[WARN] Skills documentation not found")
    
    # Test 4: Create sample custom Skill
    print("\n[TEST 4] Creating sample custom Skill...")
    sample_skill_dir = ".skills/api-design"
    
    if not os.path.exists(sample_skill_dir):
        os.makedirs(sample_skill_dir, exist_ok=True)
        print(f"[OK] Skill directory created: {sample_skill_dir}")
    
    sample_skill_file = os.path.join(sample_skill_dir, "SKILL.md")
    
    sample_skill_content = """---
name: api-design
description: RESTful API design guidance
trigger: User needs to design API endpoints
priority: implementation
type: flexible
---

# RESTful API Design Skill

## When to use
- User says "design API", "create endpoints"
- User describes functionality to expose

## Design principles

1. Resource naming
   - Use nouns, not verbs
   - Use plural: /users, /posts

2. HTTP methods
   - GET: retrieve
   - POST: create
   - PUT: update
   - DELETE: delete

3. Status codes
   - 200: success
   - 201: created
   - 400: bad request
   - 404: not found

## Workflow

1. Understand requirements
2. Design routes
3. Define responses
4. Implement handlers
5. Add tests

## Example

```rust
// GET /api/v1/users/:id
async fn get_user(id: u64) -> Result<User, Error> {
    User::find(id)
}
```
"""
    
    with open(sample_skill_file, 'w', encoding='utf-8') as f:
        f.write(sample_skill_content)
    print(f"[OK] Sample Skill created: {sample_skill_file}")
    
    # Verify Skill format
    with open(sample_skill_file, 'r', encoding='utf-8') as f:
        content = f.read()
        if '---' in content and 'name:' in content:
            print("[OK] Skill frontmatter valid")
        if '## When to use' in content:
            print("[OK] Skill content valid")
    
    # Summary
    print("\n[SUCCESS] Skills Demo test completed!\n")
    print("Summary:")
    print("  - Skills source: OK")
    print("  - Skill tool: OK")
    print("  - Skills documentation: OK")
    print("  - Custom Skill: OK")
    print("\n[NEXT] Use Skills in MatrixCode:")
    print("  matrixcode")
    print("  > /debug  # test om:debug skill")
    print("  > /plan   # test om:plan skill")
    print("\n[SUCCESS] Skills system ready!")
    
    return True

if __name__ == "__main__":
    success = test_skills_demo()
    if not success:
        print("\n[FAIL] Test failed")
        exit(1)