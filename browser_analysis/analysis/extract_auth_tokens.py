"""Extract authentication tokens from saved session."""
import json
import sys
from pathlib import Path
from typing import Dict, Any


def extract_auth_info(session_state_file: Path) -> Dict[str, Any]:
    """
    Extract authentication information from session state.

    Args:
        session_state_file: Path to session state JSON

    Returns:
        Dictionary of auth information
    """
    with open(session_state_file) as f:
        session_state = json.load(f)

    auth_info = {
        'cookies': {},
        'csrf_token': None,
        'session_cookies': []
    }

    # Extract cookies
    if 'cookies' in session_state:
        for cookie in session_state['cookies']:
            name = cookie['name']
            value = cookie['value']

            # Store important cookies
            if name in ['li_at', 'JSESSIONID', 'liap', 'bcookie']:
                auth_info['cookies'][name] = value
                auth_info['session_cookies'].append({
                    'name': name,
                    'value': value,
                    'domain': cookie.get('domain', ''),
                    'path': cookie.get('path', '/')
                })

            # Extract CSRF token if present
            if 'csrf' in name.lower():
                auth_info['csrf_token'] = value

    # Look for CSRF token in other places
    if not auth_info['csrf_token']:
        # Check if it's in cookies with ajax: prefix
        for cookie in session_state.get('cookies', []):
            if cookie['name'] == 'JSESSIONID':
                # CSRF token might be derived from JSESSIONID
                auth_info['csrf_token'] = f"ajax:{cookie['value'].split('\"')[0]}"
                break

    return auth_info


def main():
    """Main entry point."""
    if len(sys.argv) < 2:
        print("Usage: python extract_auth_tokens.py <session_state.json>")
        sys.exit(1)

    session_file = Path(sys.argv[1])
    if not session_file.exists():
        print(f"Error: File not found: {session_file}")
        sys.exit(1)

    print(f"Extracting auth tokens from {session_file}...")
    auth_info = extract_auth_info(session_file)

    # Save to output file
    output_file = Path('auth_config.json')
    with open(output_file, 'w') as f:
        json.dump(auth_info, f, indent=2)

    print(f"\n✓ Auth config extracted to {output_file}")
    print(f"\nFound cookies: {', '.join(auth_info['cookies'].keys())}")
    print(f"CSRF token: {'Yes' if auth_info['csrf_token'] else 'Not found'}")

    # Show cookie values (partially masked)
    print("\nCookie values (partially masked):")
    for name, value in auth_info['cookies'].items():
        masked_value = value[:8] + '...' + value[-4:] if len(value) > 12 else value
        print(f"  {name}: {masked_value}")


if __name__ == '__main__':
    main()
