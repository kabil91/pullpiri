# ============================================================
# Sphinx Configuration for Pullpiri Functional Safety Docs
# Mirrors the Eclipse S-CORE docs-as-code pattern.
# Requires: pip install sphinx sphinx-needs
# Build:    sphinx-build -b html docs/ _build/html/
# ============================================================

project = "Pullpiri Functional Safety"
author = "LG Electronics / S-CORE"
version = "1.0"

extensions = [
    "sphinx_needs",
]

# -- sphinx-needs: Need type definitions -----------------------------------------
# These must match the directives used in the RST files exactly.

needs_types = [
    {
        "directive": "aou_req",
        "title": "Assumption of Use",
        "prefix": "AOU_",
        "color": "#BFD8D2",
        "style": "node",
    },
    {
        "directive": "comp",
        "title": "SW Component",
        "prefix": "COMP_",
        "color": "#DCB239",
        "style": "node",
    },
    {
        "directive": "comp_req",
        "title": "Component Requirement",
        "prefix": "COMP_REQ_",
        "color": "#FEDCD2",
        "style": "node",
    },
    {
        "directive": "comp_saf_fmea",
        "title": "FMEA Failure Mode",
        "prefix": "FMEA_",
        "color": "#DF744A",
        "style": "node",
    },
    {
        "directive": "comp_saf_dfa",
        "title": "DFA Failure Initiator",
        "prefix": "DFA_",
        "color": "#DC143C",
        "style": "node",
    },
    {
        "directive": "doc",
        "title": "Safety Document",
        "prefix": "DOC_",
        "color": "#C0C0C0",
        "style": "node",
    },
]

# -- sphinx-needs: Extra option fields used in RST directives --------------------

needs_extra_options = [
    "safety_level",
    "reqtype",
    "satisfies",
    "violates",
    "mitigated_by",
    "sufficient",
    "fault_id",
    "failure_id",
    "failure_effect",
    "src_files",
    "rationale",
    "realizes",
    "safety",
    "security",
]

# Allow any ID format (the S-CORE pattern uses long double-underscore IDs)
needs_id_regex = r"^[a-zA-Z0-9_]+"

# HTML output
html_theme = "alabaster"
html_title = "Pullpiri Functional Safety Documentation"
