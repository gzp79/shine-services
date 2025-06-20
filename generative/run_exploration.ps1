# PowerShell script to run exploration-only learning

# Activate virtual environment
Write-Host "Activating virtual environment..." -ForegroundColor Green
& ".\venv\Scripts\Activate.ps1"

# Set PYTHONPATH to include the current directory
$env:PYTHONPATH = "."

# Run the exploration-only script
Write-Host "Running exploration-only learning..." -ForegroundColor Green

# Create a simple exploration script
$explorationScript = @"
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
"@

# Save and run the script
$explorationScript | Out-File -FilePath "temp_exploration.py" -Encoding UTF8
python temp_exploration.py
Remove-Item "temp_exploration.py"

# Keep the window open to see results
Write-Host "Press any key to exit..." -ForegroundColor Yellow
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown") 