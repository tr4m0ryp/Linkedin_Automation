"""Extract API patterns from captured network logs."""
import json
import sys
from pathlib import Path
from collections import defaultdict
from typing import Dict, List, Any


def extract_patterns(session_file: Path) -> Dict[str, Any]:
    """
    Extract API patterns from session summary.

    Args:
        session_file: Path to session summary JSON

    Returns:
        Dictionary of extracted patterns
    """
    with open(session_file) as f:
        session_data = json.load(f)

    patterns = {
        'endpoints': {},
        'headers': defaultdict(set),
        'payload_schemas': {},
        'auth_requirements': set()
    }

    # Read individual request files
    request_files = session_data.get('request_files', [])

    for request_file in request_files:
        if not Path(request_file).exists():
            continue

        with open(request_file) as f:
            request_data = json.load(f)

        endpoint = extract_endpoint_pattern(request_data['request']['url'])
        method = request_data['request']['method']

        # Store endpoint info
        endpoint_key = f"{method} {endpoint}"
        if endpoint_key not in patterns['endpoints']:
            patterns['endpoints'][endpoint_key] = {
                'method': method,
                'endpoint': endpoint,
                'examples': []
            }

        # Add example
        patterns['endpoints'][endpoint_key]['examples'].append({
            'url': request_data['request']['url'],
            'status': request_data['response']['status'],
            'timestamp': request_data['timestamp']
        })

        # Extract headers
        for header, value in request_data['request']['headers'].items():
            patterns['headers'][header].add(value if len(value) < 50 else '<dynamic>')

        # Extract payload schema for POST/PUT requests
        if method in ['POST', 'PUT'] and request_data['request'].get('post_data'):
            try:
                payload = json.loads(request_data['request']['post_data'])
                schema = extract_schema(payload)
                patterns['payload_schemas'][endpoint_key] = schema
            except Exception:
                pass

        # Extract auth requirements
        if 'csrf-token' in request_data['request']['headers']:
            patterns['auth_requirements'].add('csrf-token')
        if 'cookie' in request_data['request']['headers']:
            patterns['auth_requirements'].add('cookie')

    # Convert sets to lists for JSON serialization
    patterns['headers'] = {k: list(v) for k, v in patterns['headers'].items()}
    patterns['auth_requirements'] = list(patterns['auth_requirements'])

    return patterns


def extract_endpoint_pattern(url: str) -> str:
    """Extract endpoint pattern from URL."""
    try:
        if '/voyager/' in url:
            endpoint = '/voyager/' + url.split('/voyager/')[1].split('?')[0]
            # Replace IDs with placeholders
            parts = endpoint.split('/')
            pattern_parts = []
            for part in parts:
                if part.isdigit() or len(part) > 20:
                    pattern_parts.append('{id}')
                else:
                    pattern_parts.append(part)
            return '/'.join(pattern_parts)
        return url.split('?')[0]
    except Exception:
        return url


def extract_schema(obj: Any, depth: int = 0) -> Any:
    """Extract JSON schema from object."""
    if depth > 3:  # Limit recursion
        return type(obj).__name__

    if isinstance(obj, dict):
        return {k: extract_schema(v, depth + 1) for k, v in obj.items()}
    elif isinstance(obj, list):
        if obj:
            return [extract_schema(obj[0], depth + 1)]
        return []
    else:
        return type(obj).__name__


def main():
    """Main entry point."""
    if len(sys.argv) < 2:
        print("Usage: python extract_api_patterns.py <session_summary.json>")
        sys.exit(1)

    session_file = Path(sys.argv[1])
    if not session_file.exists():
        print(f"Error: File not found: {session_file}")
        sys.exit(1)

    print(f"Extracting API patterns from {session_file}...")
    patterns = extract_patterns(session_file)

    # Save to output file
    output_file = Path('api_patterns.json')
    with open(output_file, 'w') as f:
        json.dump(patterns, f, indent=2)

    print(f"\n Patterns extracted to {output_file}")
    print(f"\nFound {len(patterns['endpoints'])} unique endpoints:")
    for endpoint in patterns['endpoints'].keys():
        print(f"  • {endpoint}")

    print(f"\nAuth requirements: {', '.join(patterns['auth_requirements'])}")


if __name__ == '__main__':
    main()
