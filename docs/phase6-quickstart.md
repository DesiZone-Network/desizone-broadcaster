# Phase 6 Gateway Integration - Quick Start Guide

## Overview

The Gateway integration allows you to connect your DesiZone Broadcaster to the cloud for:
- **Remote DJ Control** - Let others control the broadcaster from anywhere
- **Real-time State Sync** - Share now playing, queue, and deck states with web services
- **AutoPilot Mode** - Hands-free broadcasting with automated playlist rotation
- **Live Talk** - Professional live microphone mode with mix-minus support

## Getting Started

### 1. Open the Gateway Panel

1. Launch DesiZone Broadcaster
2. Click the **Gateway** button in the top toolbar (Cloud icon)
3. The Gateway panel will slide in from the right

### 2. Connect to Gateway

**You'll need:**
- Gateway URL (e.g., `wss://gateway.desizone.network`)
- Authentication token (provided by your network administrator)

**Steps:**
1. Enter the Gateway URL
2. Paste your authentication token
3. Click **Connect**
4. Wait for the green connection indicator

**Status Indicators:**
- 🟢 Green dot = Connected
- 🔴 Red dot = Disconnected
- 🟡 Yellow text = Reconnecting...

### 3. Using AutoPilot

AutoPilot lets the system manage playback automatically.

**Modes:**
- **Rotation** - Uses rotation rules from the Automation page
- **Queue** - Plays tracks from the queue in order
- **Scheduled** - Follows the weekly calendar schedule

**To Enable:**
1. Select a mode (Rotation/Queue/Scheduled)
2. Click the **ON** button
3. AutoPilot will start managing playback

**When Active:**
- Current rule is displayed
- System loads and plays tracks automatically
- You can still manually control decks (AutoPilot adapts)

### 4. Managing Remote DJs

Remote DJs can control your broadcaster from the web or mobile app.

**Viewing Active Sessions:**
- Active remote DJs appear in the left column
- Shows display name, connection time, and commands sent
- Click a session to view/edit permissions

**Setting Permissions:**
1. Click a remote DJ session
2. Toggle permissions on the right:
   - ✅ **Play/Pause** - Start/stop deck playback
   - ✅ **Set Volume** - Adjust channel levels
   - ✅ **Add to Queue** - Queue new songs
   - ❌ **Load Tracks** - Load songs directly to decks (powerful)
   - ❌ **Remove from Queue** - Delete queue items
   - ❌ **Trigger Crossfade** - Force crossfade (advanced)
   - ❌ **Set AutoPilot** - Change AutoPilot settings
3. Click **Update Permissions** to save

**Kicking a Remote DJ:**
- Click the **Kick** button on their session
- Confirm the action
- They'll be disconnected immediately

**Best Practices:**
- ✅ Give minimal permissions (least privilege)
- ✅ Monitor command counts for unusual activity
- ✅ Use descriptive display names
- ❌ Don't allow "Load Tracks" unless fully trusted
- ❌ Don't give multiple DJs conflicting permissions

### 5. Live Talk Mode

Use this for professional live broadcasting with a microphone.

**Before Going Live:**
1. Configure your microphone in Settings → Voice FX
2. Test your levels with the VU meter
3. Choose your channel (Mic/Phone/VoIP)

**Mix-Minus:**
- Enable this when broadcasting phone calls
- Prevents the caller from hearing themselves (echo)
- Keep disabled for regular mic-to-air

**Going Live:**
1. Select your channel
2. Enable Mix-Minus if needed
3. Click **🎙️ GO LIVE**
4. The ON AIR indicator will pulse red
5. Speak into your microphone
6. Click **End Live Talk** when done

**Safety Tips:**
- ⚠️ Test your setup before going live
- ⚠️ Keep headphones on to prevent feedback
- ⚠️ Have a backup plan if equipment fails
- ⚠️ Never leave live talk mode unattended

## Common Scenarios

### Scenario 1: Vacation Mode
**Goal:** Let a trusted DJ run the station while you're away

1. Give them a remote DJ account
2. Set permissions:
   - ✅ Play/Pause
   - ✅ Set Volume
   - ✅ Add to Queue
   - ✅ Trigger Crossfade
   - ❌ Everything else
3. Enable AutoPilot (Rotation mode)
4. Monitor their activity via session log

### Scenario 2: Guest DJ Show
**Goal:** Let a guest host their own show

1. Schedule their show in Automation → Scheduler
2. Give them remote access with:
   - ✅ Load Tracks
   - ✅ Play/Pause
   - ✅ Set Volume
   - ✅ Add to Queue
3. They can use the web DJ panel
4. AutoPilot resumes after their show ends

### Scenario 3: Phone-In Show
**Goal:** Take live calls with professional audio quality

1. Connect phone line to audio interface
2. Select "Phone Line" as channel
3. Enable Mix-Minus
4. Click GO LIVE when caller is on
5. Your voice + caller goes to air
6. Caller doesn't hear themselves (no echo)

### Scenario 4: Full Automation
**Goal:** Run the station 24/7 without manual intervention

1. Set up rotation rules in Automation
2. Configure weekly schedule
3. Enable AutoPilot (Scheduled mode)
4. Connect to gateway for monitoring
5. Check in periodically via remote sessions

## Troubleshooting

### Connection Fails
**Symptom:** Red indicator, "Connection failed" error

**Solutions:**
1. Check your internet connection
2. Verify the gateway URL is correct (should start with `wss://`)
3. Ensure your auth token is valid (not expired)
4. Check if gateway server is online
5. Disable VPN/firewall temporarily to test

### Remote Commands Not Working
**Symptom:** Remote DJ can't control the broadcaster

**Solutions:**
1. Check their permissions (may be restricted)
2. Verify gateway connection is active (green dot)
3. Look for errors in the session log
4. Ask them to disconnect and reconnect
5. Check if AutoPilot is overriding commands

### Mix-Minus Not Working
**Symptom:** Caller hears themselves (echo)

**Solutions:**
1. Verify Mix-Minus toggle is enabled (green)
2. Check phone line is connected to correct input
3. Ensure channel is set to "Phone" not "Mic"
4. Test with headphones only (no speakers)
5. Adjust phone line input level

### AutoPilot Plays Wrong Songs
**Symptom:** Unexpected tracks being loaded

**Solutions:**
1. Check which mode is active (Rotation/Queue/Scheduled)
2. Verify rotation rules in Automation → Rotation Rules
3. Check weekly schedule for active shows
4. Ensure queue isn't empty (Queue mode)
5. Review rotation playlist contents

## Security Best Practices

### Authentication Tokens
- 🔒 Never share your auth token publicly
- 🔒 Rotate tokens every 30-90 days
- 🔒 Use different tokens for different DJs
- 🔒 Revoke tokens when staff leave

### Remote Permissions
- 🔒 Start with minimal permissions
- 🔒 Only grant "Load Tracks" to highly trusted users
- 🔒 Monitor command counts daily
- 🔒 Review session logs weekly
- 🔒 Kick suspicious sessions immediately

### Gateway Connection
- 🔒 Always use WSS (secure WebSocket)
- 🔒 Only connect to official gateway servers
- 🔒 Disconnect when not needed
- 🔒 Keep software updated

### Live Talk Safety
- 🔒 Never go live without testing first
- 🔒 Have a kill switch (End Live Talk button)
- 🔒 Monitor VU meters while live
- 🔒 Use headphones to prevent feedback

## Advanced Tips

### Monitoring Multiple Remote DJs
- Sort sessions by connection time
- Watch command counts for activity levels
- Color-code permissions mentally (green=safe, red=powerful)
- Keep notes on who has what access

### Optimizing State Sync
- VU meter sync is throttled to 200ms (adjustable in config)
- Queue updates only sent on changes (not continuous)
- Deck state pushes on play/pause/seek events
- Disconnect if you don't need real-time sync

### Custom AutoPilot Modes
- Rotation: Best for music stations with categories
- Queue: Best for request-heavy shows
- Scheduled: Best for mixed programming (talk + music)

### Professional Live Workflows
1. Prepare all materials beforehand
2. Test all equipment before going live
3. Have backup audio ready
4. Use cue points for quick navigation
5. Keep the ON AIR indicator visible at all times

## Need Help?

- 📖 Full documentation: `docs/phase6-implementation.md`
- 🐛 Report issues: Check error messages in Gateway panel
- 💬 Ask questions: Contact your network administrator
- 🔧 Advanced setup: See technical documentation

## Quick Reference

**Connection Status:**
- 🟢 Connected and syncing
- 🟡 Reconnecting...
- 🔴 Disconnected

**AutoPilot Modes:**
- 🔄 Rotation - Rule-based selection
- 📝 Queue - Sequential playback
- 📅 Scheduled - Calendar-driven

**Permission Levels:**
- 🟢 Low Risk: Play/Pause, Volume, Add to Queue
- 🟡 Medium Risk: Seek, Remove from Queue
- 🔴 High Risk: Load Tracks, Trigger Crossfade, Set AutoPilot

**Live Talk Channels:**
- 🎤 Mic - Direct microphone input
- ☎️ Phone - Phone line with mix-minus
- 💻 VoIP - Skype/Zoom/etc with mix-minus

---

**Remember:** Gateway features are powerful. Start simple and add complexity as you get comfortable!

