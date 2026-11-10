#!/bin/bash
mkdir -p /logs/verifier
pytest /tests/test_outputs.py -q && echo 1 > /logs/verifier/reward.txt || echo 0 > /logs/verifier/reward.txt
