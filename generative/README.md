# Generative Auto-Learning System

An iterative auto-learning tool that converts natural language descriptions to DSL commands, generates images, and learns from successful examples.

## Features

- **Text-to-DSL Conversion**: Uses LLM models to convert natural language to domain-specific language commands
- **Image Generation**: Custom placeholder generator that renders DSL commands into images
- **Image Description**: BLIP-2 model describes generated images
- **Similarity Analysis**: Vectorizes and compares input text with generated descriptions
- **Auto-Learning**: Stores successful examples for future learning
- **Iterative Generation**: Retries DSL generation when it fails (no hardcoded fallbacks)

## Architecture

### Core Components

1. **MistralDSLConverter**: Converts natural language to DSL commands using LLM models
2. **PlaceholderGenerator**: Renders DSL commands into images
3. **BLIPDescriptor**: Describes generated images using BLIP-2
4. **TextVectorizer**: Vectorizes text for similarity comparison
5. **ExampleLoader**: Manages example prompts for bootstrapping learning
6. **RuleManager**: Evaluates learning process and stores insights

### Learning Flow

1. Input natural language description
2. Convert to DSL commands (with retries if needed)
3. Generate image from DSL commands
4. Describe image with BLIP-2
5. Compare input text with description similarity
6. If similar enough, store as example for future learning

## Installation

### Prerequisites

- Python 3.8+
- Virtual environment (recommended)

### Setup

1. **Clone and navigate to the project**:
   ```bash
   cd generative
   ```

2. **Create virtual environment**:
   ```bash
   python -m venv venv
   ```

3. **Activate virtual environment**:
   - Windows: `.\venv\Scripts\Activate.ps1`
   - Unix/Mac: `source venv/bin/activate`

4. **Install dependencies**:
   ```bash
   pip install -r requirements.txt
   ```

5. **Set up environment**:
   ```bash
   # Windows
   .\scripts\setup_env.ps1
   
   # Unix/Mac
   ./scripts/setup_env.sh
   ```

## Usage

### Quick Start

1. **Activate virtual environment**:
   ```bash
   # Windows
   .\venv\Scripts\Activate.ps1
   
   # Unix/Mac
   source venv/bin/activate
   ```

2. **Run auto-learning**:
   ```bash
   # Set PYTHONPATH and run
   $env:PYTHONPATH = "."  # Windows
   export PYTHONPATH="."  # Unix/Mac
   python -m src.auto_learn
   ```

### Using Scripts

**Windows**:
```bash
.\run_auto_learn.ps1
```

**Unix/Mac**:
```bash
chmod +x run_auto_learn.sh
./run_auto_learn.sh
```

### Configuration

Edit `config/config.yaml` to customize:

- **Models**: Change LLM model, BLIP-2 model, vectorizer settings
- **DSL Commands**: Modify available drawing commands
- **Learning**: Adjust similarity thresholds and learning parameters
- **Output**: Configure image output directory and naming

### Example Prompts

The system uses example prompts from `data/examples/example_prompts.json` to bootstrap learning. Add your own examples:

```json
[
  {
    "input": "A red circle in the center",
    "dsl": "fill_circle(x=256, y=256, radius=50, color=\"red\")\ndraw_circle(x=256, y=256, radius=50, color=\"black\")",
    "description": "a red circle in the center of the image"
  }
]
```

## DSL Commands

The system supports these drawing commands:

- `draw_rect(x, y, w, h, color)`: Draw unfilled rectangle
- `fill_rect(x, y, w, h, color)`: Draw filled rectangle
- `draw_circle(x, y, radius, color)`: Draw unfilled circle
- `fill_circle(x, y, radius, color)`: Draw filled circle
- `draw_ellipse(x, y, w, h, color)`: Draw unfilled ellipse
- `fill_ellipse(x, y, w, h, color)`: Draw filled ellipse
- `draw_line(x1, y1, x2, y2, color, width)`: Draw line
- `draw_text(x, y, text, font_size, color)`: Draw text
- `set_background(color)`: Set background color

## Project Structure

```
generative/
├── config/
│   └── config.yaml          # Configuration file
├── data/
│   ├── examples/            # Example prompts for learning
│   ├── outputs/             # Generated images
│   └── cache/               # Model cache
├── src/
│   ├── models/              # LLM, BLIP-2, and vectorizer models
│   ├── generators/          # Image generation components
│   ├── learning/            # Example loading and learning logic
│   └── utils/               # Configuration and logging utilities
├── scripts/                 # Setup and utility scripts
├── logs/                    # Application logs
├── venv/                    # Virtual environment
├── requirements.txt         # Python dependencies
├── run_auto_learn.ps1       # Windows run script
├── run_auto_learn.sh        # Unix run script
└── README.md               # This file
```

## Learning Process

1. **Bootstrap**: System starts with example prompts from `data/examples/`
2. **Generation**: Converts input to DSL using examples as context
3. **Validation**: Generates image and describes it
4. **Evaluation**: Compares input with description similarity
5. **Learning**: Stores successful examples for future use
6. **Iteration**: Continues improving with more examples

## Troubleshooting

### Common Issues

1. **Import Errors**: Ensure PYTHONPATH is set to current directory
2. **Model Loading**: Check internet connection for model downloads
3. **Memory Issues**: Reduce batch size or use smaller models
4. **Similarity Low**: Adjust similarity threshold in config

### Logs

Check `logs/generative_ai.log` for detailed execution logs and debugging information.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Test thoroughly
5. Submit a pull request

## License

This project is licensed under the MIT License - see the LICENSE file for details. 