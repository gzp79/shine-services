#!/bin/bash

# Bash script to run exploration-only learning

echo "Activating virtual environment..."
source venv/bin/activate

# Set PYTHONPATH to include the current directory
export PYTHONPATH="."

echo "Running exploration-only learning..."

# Create a simple exploration script
cat > temp_exploration.py << 'EOF'
from src.learning.exploratory_learner import ExploratoryLearner
from src.utils.logging import setup_logging

logger = setup_logging()
logger.info("Starting exploration-only session")

exploratory_learner = ExploratoryLearner()

try:
    exploration_results = exploratory_learner.explore_and_learn(
        num_explorations=20,
        semantic_ratio=0.4
    )
    
    logger.info(f"Exploration completed: {len(exploration_results)} successful discoveries")
    
    # Show detailed results
    for result in exploration_results:
        logger.info(f"Discovery {result['exploration_id']}: {result['synthetic_input']} (confidence: {result['confidence']:.2f})")
    
    # Show final stats
    stats = exploratory_learner.get_exploration_stats()
    logger.info(f"Final stats: {stats['exploration_examples']}/{stats['total_examples']} examples from exploration")
    
except Exception as e:
    logger.error(f"Exploration failed: {e}")

logger.info("Exploration session completed")
EOF

# Run the script
python temp_exploration.py

# Clean up
rm temp_exploration.py

echo "Press any key to exit..."
read -n 1 