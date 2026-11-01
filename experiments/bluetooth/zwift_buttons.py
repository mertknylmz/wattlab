"""Identify Zwift controller button reports.

Click notifications start with 0x37. Play paddle reports start with 0x07.
Decrypting Play's AES-CCM session and turning a press into a gear change is
separate from connection management; this only recognizes the frame.
"""
CLICK_REPORT = 0x37
PLAY_REPORT = 0x07


def report_kind(payload):
    """Return 'click', 'play', or None. Never raises for a short or odd frame."""
    if not payload:
        return None
    marker = payload[0]
    if marker == CLICK_REPORT:
        return "click"
    if marker == PLAY_REPORT:
        return "play"
    return None
