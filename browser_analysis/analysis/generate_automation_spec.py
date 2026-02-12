"""Generate automation specification from captured patterns."""
import json
import sys
from pathlib import Path
from typing import Dict, Any


def generate_spec(api_patterns_file: Path, auth_config_file: Path) -> Dict[str, Any]:
    """
    Generate automation specification.

    Args:
        api_patterns_file: Path to API patterns JSON
        auth_config_file: Path to auth config JSON

    Returns:
        Automation specification
    """
    with open(api_patterns_file) as f:
        api_patterns = json.load(f)

    with open(auth_config_file) as f:
        auth_config = json.load(f)

    spec = {
        'base_url': 'https://www.linkedin.com',
        'authentication': {
            'method': 'cookie_and_csrf',
            'cookies': auth_config['cookies'],
            'csrf_token': auth_config['csrf_token']
        },
        'required_headers': {},
        'endpoints': {},
        'rate_limits': {
            'requests_per_hour': 20,
            'requests_per_day': 100,
            'min_delay_seconds': 60,
            'max_delay_seconds': 180
        }
    }

    # Extract required headers
    common_headers = {}
    for header, values in api_patterns['headers'].items():
        if header.lower() not in ['cookie', 'content-length', 'host']:
            if len(values) == 1 and '<dynamic>' not in values:
                common_headers[header] = values[0]
            else:
                common_headers[header] = '<dynamic>'

    spec['required_headers'] = common_headers

    # Extract connection endpoint
    for endpoint_key, endpoint_data in api_patterns['endpoints'].items():
        if 'invitation' in endpoint_key.lower() or 'connection' in endpoint_key.lower():
            spec['endpoints'][endpoint_key] = {
                'method': endpoint_data['method'],
                'endpoint': endpoint_data['endpoint'],
                'payload_schema': api_patterns['payload_schemas'].get(endpoint_key),
                'examples': endpoint_data['examples'][:3]  # Keep first 3 examples
            }

    return spec


def main():
    """Main entry point."""
    if len(sys.argv) < 3:
        print("Usage: python generate_automation_spec.py <api_patterns.json> <auth_config.json>")
        sys.exit(1)

    api_patterns_file = Path(sys.argv[1])
    auth_config_file = Path(sys.argv[2])

    if not api_patterns_file.exists():
        print(f"Error: File not found: {api_patterns_file}")
        sys.exit(1)

    if not auth_config_file.exists():
        print(f"Error: File not found: {auth_config_file}")
        sys.exit(1)

    print("Generating automation specification...")
    spec = generate_spec(api_patterns_file, auth_config_file)

    # Save to output file
    output_file = Path('automation_spec.json')
    with open(output_file, 'w') as f:
        json.dump(spec, f, indent=2)

    print(f"\n✓ Automation spec generated: {output_file}")
    print(f"\nFound {len(spec['endpoints'])} connection-related endpoints:")
    for endpoint in spec['endpoints'].keys():
        print(f"  • {endpoint}")

    print(f"\nRate limits configured:")
    print(f"  • {spec['rate_limits']['requests_per_hour']} requests/hour")
    print(f"  • {spec['rate_limits']['requests_per_day']} requests/day")
    print(f"  • {spec['rate_limits']['min_delay_seconds']}-{spec['rate_limits']['max_delay_seconds']}s delay")


if __name__ == '__main__':
    main()
