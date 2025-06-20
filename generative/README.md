# Generative AI Auto-Learning Tool

An iterative auto-learning system that converts natural language descriptions to DSL commands, generates images, and learns from the results to improve future generations.

## Features

- **Text-to-DSL Conversion**: Uses Mistral-7B to convert natural language to domain-specific language (DSL) commands
- **Image Generation**: Custom generator that takes DSL commands and produces images
- **Image Description**: Uses BLIP-2 to describe generated images
- **Semantic Comparison**: Vectorizes and compares input text with generated descriptions
- **Auto-Learning**: Iteratively improves by adding successful examples to the training set
- **Extensible Architecture**: Designed for easy extension and customization

## Project Structure

```
generative/
├── src/
│   ├── __init__.py
│   ├── models/
│   │   ├── __init__.py
│   │   ├── mistral_dsl.py      # Mistral-7B text-to-DSL conversion
│   │   ├── blip_descriptor.py  # BLIP-2 image description
│   │   └── vectorizer.py       # Text vectorization and comparison
│   ├── generators/
│   │   ├── __init__.py
│   │   ├── base_generator.py   # Abstract base generator
│   │   └── placeholder_generator.py  # Placeholder image generator
│   ├── learning/
│   │   ├── __init__.py
│   │   ├── rule_manager.py     # Manages learning rules and examples
│   │   └── similarity.py       # Similarity calculation and thresholds
│   ├── utils/
│   │   ├── __init__.py
│   │   ├── config.py           # Configuration management
│   │   └── logging.py          # Logging utilities
│   └── api/
│       ├── __init__.py
│       └── server.py           # FastAPI server for the system
├── data/
│   ├── examples/               # Training examples
│   ├── rules/                  # Learning rules
│   └── outputs/                # Generated images and results
├── tests/
│   ├── __init__.py
│   ├── test_models.py
│   ├── test_generators.py
│   └── test_learning.py
├── scripts/
│   ├── setup_env.ps1           # Windows setup script
│   └── setup_env.sh            # Unix/Linux setup script
├── config/
│   └── config.yaml             # Configuration file
├── requirements.txt
├── setup.py
├── env.example                 # Environment variables template
├── .gitignore
└── README.md
```

## Installation

### Prerequisites

- Python 3.8 or higher
- Git

### Quick Setup (Recommended)

#### Windows
```powershell
# Run the setup script
.\scripts\setup_env.ps1
```

#### Unix/Linux/macOS
```bash
# Make the script executable (if needed)
chmod +x scripts/setup_env.sh

# Run the setup script
./scripts/setup_env.sh
```

### Manual Setup

1. **Clone the repository** (if not already done):
```bash
git clone <repository-url>
cd generative
```

2. **Create a virtual environment**:
```bash
# Windows
python -m venv venv
venv\Scripts\activate

# Unix/Linux/macOS
python3 -m venv venv
source venv/bin/activate
```

3. **Install dependencies**:
```bash
pip install --upgrade pip
pip install -r requirements.txt
```

4. **Set up environment variables**:
```bash
# Copy the example environment file
cp env.example .env

# Edit .env with your configuration
# (Optional: modify model paths, device settings, etc.)
```

5. **Create necessary directories**:
```bash
mkdir -p data/examples data/rules data/outputs data/cache logs
```

### Environment Variables

Copy `env.example` to `.env` and configure:

- **Model Configuration**: Set model names and device preferences
- **API Configuration**: Host, port, and debug settings
- **Data Paths**: Directories for examples, rules, outputs, and cache
- **Learning Configuration**: Similarity thresholds and confidence levels

## Usage

### Basic Usage

```python
from src.models.mistral_dsl import MistralDSLConverter
from src.generators.placeholder_generator import PlaceholderGenerator
from src.models.blip_descriptor import BLIPDescriptor
from src.learning.rule_manager import RuleManager

# Initialize components
dsl_converter = MistralDSLConverter()
generator = PlaceholderGenerator()
descriptor = BLIPDescriptor()
rule_manager = RuleManager()

# Process a text input
input_text = "A red cat sitting on a blue chair"
dsl_commands = dsl_converter.convert(input_text)
generated_image = generator.generate(dsl_commands)
description = descriptor.describe(generated_image)

# Learn from the result
rule_manager.evaluate_and_learn(input_text, dsl_commands, description)
```

### Running the Auto-Learning Loop

```bash
# Activate virtual environment first
# Windows: venv\Scripts\activate
# Unix/Linux/macOS: source venv/bin/activate

# Run the auto-learning process
python src/auto_learn.py
```

### Running the API Server

```bash
# Activate virtual environment first
python -m src.api.server
```

### Running Tests

```bash
# Activate virtual environment first
pytest tests/
```

## Configuration

The system can be configured through `config/config.yaml`:

```yaml
models:
  mistral:
    model_name: "mistralai/Mistral-7B-Instruct-v0.2"
    max_length: 512
    temperature: 0.7
  
  blip:
    model_name: "Salesforce/blip2-opt-2.7b"
    max_length: 100

generator:
  type: "placeholder"
  output_dir: "data/outputs"

learning:
  similarity_threshold: 0.8
  max_examples_per_rule: 100
  min_confidence: 0.7

vectorizer:
  model_name: "sentence-transformers/all-MiniLM-L6-v2"
  similarity_metric: "cosine"
```

## DSL Commands

The system supports the following DSL commands:

- `draw_rect(x=10, y=20, w=100, h=50, color="red")` - Draw a rectangle
- `draw_circle(x=200, y=150, radius=40, color="blue")` - Draw a circle

## Extending the System

### Adding New Generators

1. Create a new generator class inheriting from `BaseGenerator`:
```python
from src.generators.base_generator import BaseGenerator

class MyCustomGenerator(BaseGenerator):
    def generate(self, dsl_commands: str) -> Image:
        # Your generation logic here
        pass
```

2. Register it in the configuration:
```yaml
generator:
  type: "my_custom"
  # Additional parameters
```

### Adding New Learning Rules

1. Extend the `RuleManager` class:
```python
class MyCustomRuleManager(RuleManager):
    def evaluate_similarity(self, input_text: str, output_description: str) -> float:
        # Your custom similarity logic
        pass
```

## Troubleshooting

### Common Issues

1. **CUDA out of memory**: Reduce batch size or use CPU device
2. **Model download issues**: Check internet connection and disk space
3. **Import errors**: Ensure virtual environment is activated

### Performance Tips

- Use GPU if available (set `DEVICE=cuda` in `.env`)
- Adjust model parameters in `config.yaml` for your hardware
- Use smaller models for faster inference

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## License

MIT License - see LICENSE file for details. 