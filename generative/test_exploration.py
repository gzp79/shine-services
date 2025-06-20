#!/usr/bin/env python3
"""Test script for exploratory learning with BLIP confidence scores."""

from src.learning.exploratory_learner import ExploratoryLearner
from src.utils.logging import setup_logging

def main():
    logger = setup_logging()
    logger.info("Starting exploration test with BLIP confidence")
    
    print("Creating ExploratoryLearner...")
    learner = ExploratoryLearner()
    print("ExploratoryLearner created successfully")
    
    try:
        print("Starting exploration with 3 attempts...")
        # Run a small exploration with lower confidence threshold for testing
        results = learner.explore_and_learn(
            num_explorations=3,
            semantic_ratio=0.5,
            confidence_threshold=0.3  # Very low threshold for testing
        )
        
        print(f"Exploration completed: {len(results)} discoveries")
        
        # Show results
        for result in results:
            print(f"Discovery {result['exploration_id']}: {result['synthetic_input']} (BLIP confidence: {result['blip_confidence']:.3f})")
        
        # Show stats
        stats = learner.get_exploration_stats()
        print(f"Stats: {stats['exploration_examples']}/{stats['total_examples']} examples from exploration")
        
    except Exception as e:
        print(f"Exploration failed: {e}")
        import traceback
        traceback.print_exc()

if __name__ == "__main__":
    main() 