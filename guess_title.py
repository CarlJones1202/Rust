"""
Guess the title of a vipergirls.to gallery from its URL.

The URL slug contains the title mixed with metadata noise:
  - Image counts: x60, x120, 80-pictures, 62-Photos, etc.
  - Resolutions: 5000px, 4480x6720, 3540x4720, 14000px, etc.
  - Dates: (02-02-25), (08-23-2024), (29-Mar-2024), Nov-16-2021, 2023-03-30
  - File info: 120-Jpg, 92-Jpg, MB sizes
  - Site prefixes: FemJoy-com, Hegre
  - Misc: hi-res, pre-release, Full-Set, Upcoming-Release, Pix

Strategy:
  1. Extract the slug from the URL (between thread ID and query params)
  2. Pre-clean the slug by removing parenthesized metadata blocks and inline dates
  3. Strip leading dates (YYYY-MM-DD or DD-MM-YYYY patterns at start)
  4. Remove metadata tokens (counts, resolutions, dates, file info)
  5. Strip trailing numeric remnants
  6. The remaining tokens form "ModelName - GalleryTitle"
"""

import re
import sys
import json
from urllib.parse import unquote


def guess_title(url: str) -> str | None:
    """Given a vipergirls.to URL, guess the gallery title.
    
    Returns a string like "Julietta - My Flower" or None if not a vipergirls URL.
    """
    # Only handle vipergirls URLs
    if "vipergirls.to/threads/" not in url:
        return None
    
    # Extract the slug: everything after /threads/{id}- and before ? or # or &
    match = re.search(r'/threads/\d+-(.*?)(?:\?|#|&|$)', url)
    if not match:
        return None
    
    slug = match.group(1)
    
    # URL-decode any percent-encoded chars (e.g., %E2%80%93 = –)
    slug = unquote(slug)
    
    # --- Phase 0: Pre-clean the slug ---
    slug = _preclean_slug(slug)
    
    # Split on hyphens
    parts = slug.split('-')
    
    # --- Phase 1: Strip leading date prefix ---
    parts = _strip_leading_date(parts)
    
    # --- Phase 2: Remove site prefixes ---
    parts = _strip_site_prefix(parts)
    
    # --- Phase 3: Remove noise tokens ---
    cleaned = _remove_noise(parts)
    
    # --- Phase 4: Strip trailing numeric remnants ---
    cleaned = _strip_trailing_numbers(cleaned)
    
    # --- Phase 5: Identify model name vs gallery title ---
    return _format_title(cleaned)


def _preclean_slug(slug: str) -> str:
    """Remove parenthesized metadata blocks and inline date sequences."""
    
    # Remove parenthesized blocks containing dates, counts, dimensions
    slug = re.sub(r'\([^)]*\)', '', slug)
    
    # Remove inline date sequences (not in parens):
    # "Nov-04-2023", "Oct-04-2022", "Dec-14-2021", "Jul-13-2021", etc.
    slug = re.sub(r'-(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)-\d{1,2}-\d{4}', '', slug)
    # "January-31-2015", "February-2-2017", etc.
    slug = re.sub(r'-(?:January|February|March|April|May|June|July|August|September|October|November|December)-\d{1,2}-\d{4}', '', slug)
    
    # Date at end: "DD-Mon-YYYY" or "Mon-DD-YYYY" without leading hyphen
    slug = re.sub(r'-\d{1,2}-\d{1,2}-\d{2,4}$', '', slug)
    
    # "Aug-09-2019" style at end
    slug = re.sub(r'-[A-Z][a-z]{2}-\d{1,2}-\d{2,4}$', '', slug)
    
    # YYYY-MM-DD at end (standalone): "-2024-10-02", "-2020-07-04"
    slug = re.sub(r'-\d{4}-\d{2}-\d{2}$', '', slug)
    
    # Full inline dates like "Sep-12-2020" in middle of slug
    slug = re.sub(r'-[A-Z][a-z]{2,8}-\d{1,2}-\d{4}', '', slug)
    
    # "Ot-23-2020" (typo for Oct)
    slug = re.sub(r'-Ot-\d{1,2}-\d{4}', '', slug)
    
    # Dates like "08-23-2024" without month names (in non-paren context at end)
    slug = re.sub(r'-\d{2}-\d{2}-\d{4}$', '', slug)
    
    # Remove compound noise tokens like "71Photos", "80pictures", etc.
    slug = re.sub(r'-\d+Photos\b', '', slug, flags=re.IGNORECASE)
    slug = re.sub(r'-\d+pictures\b', '', slug, flags=re.IGNORECASE)
    slug = re.sub(r'-\d+pics\b', '', slug, flags=re.IGNORECASE)
    slug = re.sub(r'\b\d+Photos-', '-', slug, flags=re.IGNORECASE)
    
    # Clean up resulting double-hyphens or leading/trailing hyphens
    slug = re.sub(r'-{2,}', '-', slug)
    slug = slug.strip('-')
    
    return slug


def _strip_leading_date(parts: list[str]) -> list[str]:
    """Remove leading date patterns like 2023-03-30 or YY-MM-DD from parts."""
    if len(parts) < 4:
        return parts
    
    first = parts[0].strip('()')
    
    # Check for YYYY-MM-DD at start
    if (re.match(r'^\d{4}$', first) and 
        len(parts) > 2 and 
        re.match(r'^\d{2}$', parts[1]) and 
        re.match(r'^\d{2}\)?$', parts[2])):
        return parts[3:]
    
    # Check for YY-MM-DD at start (e.g., "23-02-01", "10-10-2020")
    if (re.match(r'^\d{2}$', first) and 
        len(parts) > 2 and 
        re.match(r'^\d{2}$', parts[1]) and 
        re.match(r'^\d{2,4}\)?$', parts[2])):
        # Check if the rest looks like a name (starts with capital letter)
        rest_start = parts[3] if len(parts) > 3 else ""
        if rest_start and rest_start[0].isupper():
            return parts[3:]
    
    return parts


def _strip_site_prefix(parts: list[str]) -> list[str]:
    """Remove site prefixes like FemJoy-com, Hegre, Unpublished, Femjoy."""
    if not parts:
        return parts
    
    # "FemJoy-com" -> strip first two parts
    if parts[0] == 'FemJoy' and len(parts) > 1 and parts[1].lower() == 'com':
        return parts[2:]
    
    # "Hegre" alone at start
    if parts[0] == 'Hegre' and len(parts) > 1:
        return parts[1:]
    
    # "Unpublished" at start
    if parts[0] == 'Unpublished' and len(parts) > 1:
        return parts[1:]
    
    # "Femjoy" at start
    if parts[0] == 'Femjoy' and len(parts) > 1:
        return parts[1:]
    
    return parts


def _is_noise(token: str) -> bool:
    """Check if a token is metadata noise."""
    clean = token.strip('()')
    
    if not clean:
        return True
    
    # Exact noise words (case-insensitive)
    noise_words = {
        'pictures', 'photos', 'pics', 'images', 'pix',
        'jpg', 'mb', 'hi', 'res', 'pre', 'release',
        'upcoming', 'full', 'set', 'card',
    }
    if clean.lower() in noise_words:
        return True
    
    # x{N} or {N}x patterns (image counts)
    if re.match(r'^[xX]\d+$', clean):
        return True
    if re.match(r'^\d+[xX]$', clean):
        return True
    
    # Resolution: {N}px
    if re.match(r'^\d+px$', clean, re.IGNORECASE):
        return True
    
    # Dimensions: {N}x{N}, optionally with px suffix
    if re.match(r'^\d+[xX]\d+(px)?$', clean, re.IGNORECASE):
        return True
    
    # Standalone "X" or "x" (from dimension patterns)
    if clean in ('X', 'x'):
        return True
    
    # Bare numbers >= 10 are almost always metadata
    if re.match(r'^\d+$', clean):
        num = int(clean)
        if num >= 10:
            return True
    
    # Month names (full and abbreviated)
    months_short = {'Jan', 'Feb', 'Mar', 'Apr', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'}
    months_long = {'January', 'February', 'March', 'April', 'June', 'July', 'August', 
                   'September', 'October', 'November', 'December'}
    if clean in months_short or clean in months_long:
        return True
    
    # Card ID patterns: e1962, f0892, e2105
    if re.match(r'^[ef]\d+$', clean):
        return True
    
    # Resolution shorthand: "4000p"
    if re.match(r'^\d+p$', clean):
        return True
    
    # Special characters
    if clean in ('*', '–', '\u2013'):
        return True
    
    # "MP" (megapixels)
    if clean == 'MP':
        return True
    
    return False


def _remove_noise(parts: list[str]) -> list[str]:
    """Remove noise tokens while preserving the title structure."""
    result = []
    i = 0
    while i < len(parts):
        token = parts[i]
        clean = token.strip('()')
        
        if not clean:
            i += 1
            continue
        
        # Keep "May" if it's likely a name (part of "Monika May") 
        # but skip if it's a month in a date context
        if clean == 'May':
            prev_is_num = (i > 0 and re.match(r'^\d+\)?$', parts[i-1].strip('()')))
            next_is_num = (i + 1 < len(parts) and re.match(r'^\(?\d+$', parts[i+1].strip('()')))
            if prev_is_num or next_is_num:
                i += 1
                continue
            result.append(clean)
            i += 1
            continue
        
        # Check general noise
        if _is_noise(token):
            i += 1
            continue
        
        # HTML entity "&amp;" -> "&"
        if clean == 'amp':
            result.append('&')
            i += 1
            continue
        
        # Unicode em-dash — decorative separator
        if clean in ('–', '\u2013'):
            i += 1
            continue
        
        result.append(clean)
        i += 1
    
    return result


def _strip_trailing_numbers(parts: list[str]) -> list[str]:
    """Remove trailing single/double-digit numbers that are date remnants."""
    while parts and re.match(r'^\d{1,2}$', parts[-1]):
        # Don't strip if it looks intentional (e.g., "Part 2", "Volume 1")
        # Heuristic: keep if the previous word suggests it's a sequence number
        if len(parts) >= 2:
            original_prev = parts[-2]
            prev_lower = original_prev.lower()
            # These words suggest the number is intentional
            if prev_lower in ('part', 'vol', 'volume', 'chapter', 'set', 'ii', 'iii',
                         'door', 'circles', 'rambler', 'life', 'overdrive'):
                break
            # If previous word is a title word and number is 1-4, it might be 
            # a set number like "Shany 2" or "Escape 1"
            num = int(parts[-1])
            if num <= 4 and original_prev[0].isupper():
                break
        parts = parts[:-1]
    
    return parts


# Known model names
_KNOWN_MODELS = {
    'Julietta',
    'Monika May',
    'Nerina',
    'Kira Rami',
    'Carolina K',
    'Libby',
    'Michaela Isizzu',
    'Kalena',
    'Kalena A',
    'Serena Wood',
    'Alice Nekrasova',
    'Alice',
    'Hareniks',
    'Rinna Ly',
    'Gloria Sol',
    'Mary Rock',
    'Lily Chey',
    'Lilii',
    'Hope Heaven',
    'Elly Clutch',
    'Yarina P',
    'Yarina A',
    'Edessa G',
    'Danica',
    'Danica Jewels',
    'Tavia',
    'Niemira',
    'Neimera',
    'Guerlain',
    'Angelique Lapiedra',
    'Lacy Lennon',
    'Jeff Milton',
    'SofieQ',
    'Dolores',
    'Skye Blue',
    'Amelia',
    'Shany',
    'Anna L',
    'Anna S',
    'Valery',
    'Lily C',
    'Indiana A',
    'Anelie A',
    'Liz Ocean',
    'Charlotte Grey',
    'Danna',
    'Danna Bliss',
    'Chloe Moss',
    'Arya Fae',
    'Emily Bloom',
    'Paula Shy',
    'Nancy A',
    'Ulia',
    'Iness',
    'Tali Dova',
    'Apolonia',
    'Sabrisse',
    'Sabrisse A',
    'Savanna Rose',
    'Kelly Collins',
    'Cappello',
    'Antonio Clemens',
    'Leya Desantis',
}

_SORTED_MODELS = sorted(_KNOWN_MODELS, key=len, reverse=True)


def _find_model_name(parts: list[str]) -> tuple[str | None, int]:
    """Try to find a known model name at the start of parts."""
    joined = ' '.join(parts)
    
    # Exact match first
    for model in _SORTED_MODELS:
        if joined.startswith(model):
            model_parts = model.split(' ')
            if len(model_parts) <= len(parts):
                if all(parts[i] == model_parts[i] for i in range(len(model_parts))):
                    return model, len(model_parts)
    
    # Case-insensitive fallback (for lowercase slugs like "yarina-a-adesi")
    joined_lower = joined.lower()
    for model in _SORTED_MODELS:
        if joined_lower.startswith(model.lower()):
            model_parts = model.split(' ')
            if len(model_parts) <= len(parts):
                if all(parts[i].lower() == model_parts[i].lower() for i in range(len(model_parts))):
                    return model, len(model_parts)
    
    return None, 0


def _is_model_name_word(word: str) -> bool:
    """Check if a word looks like it could be part of a model name."""
    clean = word.strip('()')
    return bool(clean) and clean[0].isupper() and clean.isalpha()


def _format_title(parts: list[str]) -> str:
    """Format cleaned parts into "Model Name - Gallery Title"."""
    if not parts:
        return "Unknown"
    
    model_name, consumed = _find_model_name(parts)
    
    if model_name and consumed < len(parts):
        title_parts = parts[consumed:]
        title = ' '.join(title_parts)
        return f"{model_name} - {title}"
    elif model_name:
        return model_name
    
    # Fallback: heuristic — first 1-3 capitalized words are the model name
    model_parts = []
    title_start = 0
    
    for i, part in enumerate(parts):
        if _is_model_name_word(part) and i < 4:
            model_parts.append(part)
            title_start = i + 1
        else:
            break
    
    if model_parts and title_start < len(parts):
        model = ' '.join(model_parts)
        title = ' '.join(parts[title_start:])
        return f"{model} - {title}"
    else:
        return ' '.join(parts)


def process_file(filepath: str):
    """Process a file of URLs and print guessed titles."""
    if filepath == '-':
        import sys
        f = sys.stdin
    else:
        f = open(filepath, 'r')
    
    try:
        for line in f:
            url = line.strip()
            if not url:
                continue
            title = guess_title(url)
            if title is not None:
                print(f"{title}")
            else:
                print(f"[SKIP] {url}")
    finally:
        if filepath != '-':
            f.close()


def process_urls(urls: list[str]) -> list[dict]:
    """Process a list of URLs and return results as dicts."""
    results = []
    for url in urls:
        url = url.strip()
        if not url:
            continue
        title = guess_title(url)
        results.append({
            "url": url,
            "guessed_title": title,
        })
    return results


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python guess_title.py <file_or_url>")
        print("  file: path to a file containing URLs (one per line)")
        print("  url:  a single vipergirls.to URL")
        sys.exit(1)
    
    arg = sys.argv[1]
    
    if arg.startswith('http'):
        result = guess_title(arg)
        if result:
            print(result)
        else:
            print(f"Could not parse: {arg}")
    else:
        process_file(arg)
