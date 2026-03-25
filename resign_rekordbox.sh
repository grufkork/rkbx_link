#!/bin/bash

set -e

echo "=== Resigning Rekordbox with get-task-allow ==="
echo ""
echo "This will:"
echo "  1. Extract current Rekordbox entitlements"
echo "  2. Add get-task-allow entitlement"
echo "  3. Remove Apple's notarization and re-sign"
echo ""
echo "WARNING: Rekordbox will no longer be notarized by Apple after this."
echo "         You may get security warnings when launching it."
echo ""
read -p "Continue? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Cancelled."
    exit 1
fi

echo ""
echo "Step 1: Extracting current entitlements from Rekordbox..."
TEMP_ENTITLEMENTS=$(mktemp ./rekordbox_entitlements.XXXXXX.plist)

# Check if Rekordbox is signed
if codesign -d "/Applications/rekordbox 7/rekordbox.app" 2>&1 | grep -q "code object is not signed"; then
    echo "✗ Rekordbox is already unsigned"
    echo ""
    echo "This script can only extract entitlements from a signed Rekordbox."
    echo "To fix this:"
    echo "  1. Reinstall Rekordbox from Pioneer's website"
    echo "  2. Run this script again before launching Rekordbox"
    echo ""
    echo "Alternatively, if you know Rekordbox already has get-task-allow,"
    echo "you don't need to run this script again."
    rm "$TEMP_ENTITLEMENTS"
    exit 1
fi

# Extract existing entitlements
if ! codesign -d --entitlements :- "/Applications/rekordbox 7/rekordbox.app" 2>/dev/null > "$TEMP_ENTITLEMENTS"; then
    echo "✗ Failed to extract entitlements from Rekordbox"
    echo "  Make sure Rekordbox is installed at: /Applications/rekordbox 7/rekordbox.app"
    rm "$TEMP_ENTITLEMENTS"
    exit 1
fi

echo "✓ Extracted existing entitlements"

# Check if get-task-allow is already present
if grep -q "com.apple.security.get-task-allow" "$TEMP_ENTITLEMENTS"; then
    echo "✓ get-task-allow already present in entitlements"
else
    echo "Adding get-task-allow to entitlements..."
    # Insert get-task-allow before closing </dict>
    sed -i '' 's|</dict>|    <key>com.apple.security.get-task-allow</key>\
    <true/>\
</dict>|' "$TEMP_ENTITLEMENTS"
fi

echo ""
echo "Step 2: Removing existing signature (if present)..."
sudo codesign --remove-signature "/Applications/rekordbox 7/rekordbox.app" 2>/dev/null || echo "  (No signature to remove)"

echo ""
echo "Step 3: Re-signing with updated entitlements..."
sudo codesign -s - --deep --force --entitlements "$TEMP_ENTITLEMENTS" "/Applications/rekordbox 7/rekordbox.app"

# Clean up temp file
rm "$TEMP_ENTITLEMENTS"

echo ""
echo "Step 4: Verifying new signature..."
if codesign -d --entitlements - "/Applications/rekordbox 7/rekordbox.app/Contents/MacOS/rekordbox" 2>&1 | grep -q "get-task-allow"; then
    echo "✓ get-task-allow is present!"
else
    echo "✗ Failed to add entitlement"
    exit 1
fi

echo ""
echo "Step 5: Testing notarization status..."
spctl -a -vv "/Applications/rekordbox 7/rekordbox.app" 2>&1 || echo "(Rekordbox is no longer notarized - this is expected)"

echo ""
echo "=== Done! ==="
echo ""
echo "Now test:"
echo "  1. Library injection should work (no longer notarized)"
echo "  2. Memory reading should work (has get-task-allow)"
echo ""
echo "Try running: sudo target/release/rkbx_link"
