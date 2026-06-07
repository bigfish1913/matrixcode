#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Run all demo tests"""

import os
import sys
import subprocess

# Handle Windows encoding
if sys.platform == 'win32':
    import io
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')

def run_test(test_file):
    """Run a single test file"""
    print(f"\n{'='*60}")
    print(f"[RUNNING] {test_file}")
    print('='*60)
    
    result = subprocess.run(
        ["python", test_file],
        capture_output=True,
        text=True,
        encoding='utf-8'
    )
    
    print(result.stdout)
    if result.stderr:
        print("[STDERR]", result.stderr)
    
    return result.returncode == 0

def main():
    """Run all demo tests"""
    print("[TEST] Running all MatrixCode Demo tests...\n")
    
    test_files = [
        "test-cli-basic.py",
        "test-mcp-integration.py",
        "test-skills.py",
        "test-workflows.py",
    ]
    
    results = {}
    
    for test_file in test_files:
        if os.path.exists(test_file):
            success = run_test(test_file)
            results[test_file] = success
        else:
            print(f"\n[SKIP] {test_file} not found")
            results[test_file] = False
    
    # Summary
    print("\n" + "="*60)
    print("[SUMMARY] All Demo Tests Results")
    print("="*60)
    
    total = len(results)
    passed = sum(1 for v in results.values() if v)
    
    print("\nResults:")
    for test_file, success in results.items():
        status = "[PASS]" if success else "[FAIL]"
        print(f"  {status} {test_file}")
    
    print(f"\nTotal: {passed}/{total} tests passed")
    
    if passed == total:
        print("\n[SUCCESS] All demos verified!")
        print("\n[NEXT] Update website files with demo content")
        return True
    else:
        print("\n[FAIL] Some tests failed")
        print("[INFO] Check individual test outputs for details")
        return False

if __name__ == "__main__":
    success = main()
    if not success:
        exit(1)