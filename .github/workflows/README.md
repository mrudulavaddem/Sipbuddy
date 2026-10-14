# Sip Buddy 💧

A tiny desktop app for Mac and Windows. On the schedule you set, a cheerful character walks across the bottom of your screen with her water bottle and reminds you to have a drink. She appears over any app, even when Chrome is closed.

![Sip Buddy preview](docs/preview.png)

- Click **Done** to log a glass (she has a happy sip and walks off).
- Click **Snooze 10 min** to get her to come back later.
- If you ignore her for 2 minutes, she snoozes on her own.
- The rest of your screen stays fully clickable while she's there.
- Everything else lives in the water-drop icon in your menu bar (Mac) or system tray (Windows): **Remind me now**, **I just drank a glass**, **Pause**, **Settings** and **Quit**.
- In Settings you can choose the interval, active hours, weekdays only, daily goal, a gentle chime and whether it starts automatically.

It's about 2 MB to download, uses very little memory, and keeps all data on your own computer. There's no account and nothing is sent anywhere.

---

## Part 1: Build the installers (one-off, about 15 minutes)

GitHub builds the Mac and Windows installers for you for free.

1. Sign in to **github.com** with your personal account.
2. Click **+** (top right) → **New repository**. Name it `sip-buddy`, choose **Private** (or Public if you'd like to share the link) and click **Create repository**.
3. On the new repository page, click **uploading an existing file**.
4. Unzip `sip-buddy.zip` on your computer. Open the `sip-buddy` folder, select **everything inside it** (including the hidden `.github` folder; see the tip below) and drag it into the browser. Click **Commit changes**.
5. Go to the **Actions** tab. If asked, click **I understand my workflows, go ahead and enable them**.
6. Click **Build installers** on the left → **Run workflow** → **Run workflow**.
7. Wait about 10–15 minutes until both jobs show a green tick.
8. Go to the **Releases** section on the right of the repository's main page and open **Sip Buddy v1.0.0**. Your installers are there:
   - `Sip Buddy_1.0.0_x64-setup.exe` (Windows)
   - `Sip Buddy_1.0.0_universal.dmg` (Mac, for both Intel and Apple Silicon)

> **Tip: the `.github` folder is hidden.** On a Mac, press **Cmd + Shift + .** in Finder to show hidden files. On Windows, go to File Explorer → **View** → **Show** → **Hidden items**. If the upload skips it, you can create the file manually instead: in your repository, click **Add file → Create new file**, name it `.github/workflows/build.yml` and paste in the contents of that file.

To share with colleagues, send them the Releases link (if the repository is public) or download the two files and share them via Google Drive.

---

## Part 2: Install it (send this part to colleagues)

Sip Buddy isn't signed with a paid Apple or Microsoft developer certificate, so your computer will show a one-off warning the first time. That's expected.

### Windows

1. Download `Sip Buddy_..._x64-setup.exe` and double-click it.
2. If you see **"Windows protected your PC"**, click **More info** → **Run anyway**.
3. Click through the installer. No admin rights are needed.
4. Sip Buddy starts automatically. Look for the water-drop icon near the clock (it may be hidden under the **^** arrow; drag it out to keep it visible).

### Mac

1. Download `Sip Buddy_..._universal.dmg`, open it and drag **Sip Buddy** into **Applications**.
2. Open Sip Buddy from Applications. When macOS says it can't verify the app, click **Done** (or **Cancel**).
3. Open **System Settings → Privacy & Security**, scroll down and click **Open Anyway** next to Sip Buddy. Confirm with your password.
4. The water-drop icon appears in the menu bar at the top of the screen.

If the Mac says the app "is damaged", open **Terminal** and run the following command, then open the app again:

```
xattr -cr "/Applications/Sip Buddy.app"
```

### First run

The Settings window opens so you can set your schedule. By default, she visits every 45 minutes between 9 am and 6 pm, with a goal of 8 glasses, and the app starts when your computer starts. Click **Show me now** to meet her.

---

## Updating the app later

1. In your repository, edit `src-tauri/tauri.conf.json` and change `"version": "1.0.0"` to `"1.0.1"`.
2. Go to **Actions → Build installers → Run workflow**. A new release with fresh installers appears in about 15 minutes.

## For the technically curious

- Built with [Tauri 2](https://tauri.app). It uses the computer's built-in web engine, so it's tiny compared with Electron apps.
- `src/`: the character (hand-drawn SVG with CSS walk animation), the overlay and the settings screen.
- `src-tauri/src/lib.rs`: the timer, tray menu, click-through overlay and saved settings.
- Settings and today's count are stored as small JSON files in the app's config folder.
- To build locally: install Rust and Node, then run `npm install` and `npm run tauri build`. Launching the app with `--remind-now` shows a reminder straight away.
