import { LockOpen } from "@gravity-ui/icons";
import { AlertDialog, Button } from "@heroui/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { downloadDir, join } from "@tauri-apps/api/path";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useRef, useState } from "react";

import type { DownloadItem, DownloadProgress } from "./types";

import { DownloadControls } from "./components/DownloadControls";
import { DownloadQueueTable } from "./components/DownloadQueueTable";
import { StatusFooter } from "./components/StatusFooter";
import {
  extractUsernameFromUrl,
  isTikTokProfileUrl,
  isTikTokVideoUrl,
  sanitizePathSegment,
} from "./utils";

function App() {
  const buyMeACoffeeUrl = "https://buymeacoffee.com/toandev95";
  const cookiesGuideUrl =
    "https://github.com/The-Senile-Developers/TikTok-Bulk-Downloader";

  const [currentQueueItem, setCurrentQueueItem] = useState(0);
  const [downloadItems, setDownloadItems] = useState<DownloadItem[]>([]);
  const [queueTotal, setQueueTotal] = useState(0);
  const [urlInputValue, setUrlInputValue] = useState("");
  const [isBatchRunning, setIsBatchRunning] = useState(false);
  const [isPaused, setIsPaused] = useState(false);
  const [currentPage, setCurrentPage] = useState(1);
  const [downloadStatus, setDownloadStatus] = useState("ready");
  const [statusMessage, setStatusMessage] = useState("Ready");
  const [isMissingCookiesModalOpen, setIsMissingCookiesModalOpen] =
    useState(false);

  const activeQueueItemRef = useRef(0);
  const cancelRequestedRef = useRef(false);

  const isDownloadActive =
    isBatchRunning ||
    downloadStatus === "starting" ||
    downloadStatus === "downloading" ||
    downloadStatus === "paused";

  useEffect(() => {
    let mounted = true;

    const validateCookiesAtStartup = async () => {
      try {
        const hasCookies = await invoke<boolean>("has_cookies_file");
        if (!mounted) return;

        if (!hasCookies) {
          setIsMissingCookiesModalOpen(true);
          setStatusMessage(
            "Missing or empty cookies.txt. Open the guide, then close and reopen the app.",
          );
        }
      } catch (error) {
        if (!mounted) return;

        setIsMissingCookiesModalOpen(true);
        setStatusMessage(`Could not validate cookies file: ${String(error)}`);
      }
    };

    validateCookiesAtStartup();

    return () => {
      mounted = false;
    };
  }, []);

  useEffect(() => {
    let mounted = true;

    const subscribeToProgress = async () => {
      const unlisten = await listen<DownloadProgress>(
        "download-progress",
        (event) => {
          if (!mounted) return;

          const payload = event.payload;
          setDownloadStatus(payload.status);

          if (payload.status === "paused") {
            setIsPaused(true);
          }

          if (payload.status === "downloading" || payload.status === "done") {
            setIsPaused(false);
          }

          if (payload.status === "cancelled") {
            const activeItemIndex = activeQueueItemRef.current - 1;
            setDownloadItems((prev) => {
              const next = [...prev];
              if (next[activeItemIndex]) {
                next[activeItemIndex] = {
                  ...next[activeItemIndex],
                  status: "cancelled",
                };
              }
              return next;
            });
            setStatusMessage(payload.detail || "Cancelled");
            return;
          }

          if (payload.status === "done") {
            const activeItemIndex = activeQueueItemRef.current - 1;
            setDownloadItems((prev) => {
              const next = [...prev];
              if (next[activeItemIndex]) {
                next[activeItemIndex] = {
                  ...next[activeItemIndex],
                  progress: 100,
                  status: "done",
                };
              }
              return next;
            });
            setStatusMessage(payload.detail || "Done");
            return;
          }

          if (payload.status.startsWith("log:")) {
            setStatusMessage(payload.status.slice(4));
            return;
          }

          const activeItemIndex = activeQueueItemRef.current - 1;
          setDownloadItems((prev) => {
            const next = [...prev];
            if (next[activeItemIndex]) {
              next[activeItemIndex] = {
                ...next[activeItemIndex],
                description:
                  payload.current_title || next[activeItemIndex].description,
                progress: payload.percent ?? next[activeItemIndex].progress,
                status: payload.status,
              };
            }
            return next;
          });
          setStatusMessage(
            payload.detail ||
              `${payload.percent}% | ${payload.speed} | ETA ${payload.eta}`,
          );
        },
      );

      return unlisten;
    };

    let cleanup: (() => void) | undefined;
    subscribeToProgress().then((unlisten) => {
      cleanup = unlisten;
    });

    return () => {
      mounted = false;
      cleanup?.();
    };
  }, []);

  const quitApp = async () => {
    try {
      await invoke("quit_app");
    } catch (error) {
      setStatusMessage(`Failed to close app: ${String(error)}`);
    }
  };

  const resolveDownloadCommand = (url: string) => {
    if (isTikTokVideoUrl(url)) return "download_video";
    if (isTikTokProfileUrl(url)) return "download_profile";
    return null;
  };

  const resolveDownloadFolderPath = async (url?: string) => {
    const rootDownloadDir = await downloadDir();

    if (!url) {
      return rootDownloadDir;
    }

    const username = extractUsernameFromUrl(url) ?? "unknown";
    return join(rootDownloadDir, sanitizePathSegment(username));
  };

  const startDownloadQueue = async () => {
    const urls = urlInputValue
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);

    if (urls.length === 0) {
      setDownloadStatus("error");
      setStatusMessage("Please enter at least one URL.");
      return;
    }

    const items: DownloadItem[] = urls.map((url, index) => ({
      description: url,
      id: index + 1,
      link: url,
      progress: 0,
      status: "pending",
    }));

    cancelRequestedRef.current = false;
    setIsPaused(false);
    setDownloadItems(items);
    setCurrentPage(1);
    setIsBatchRunning(true);
    setCurrentQueueItem(0);
    setQueueTotal(urls.length);
    setStatusMessage(`Starting ${urls.length} URL(s)...`);

    const failedUrls: string[] = [];

    for (let i = 0; i < urls.length; i += 1) {
      if (cancelRequestedRef.current) {
        break;
      }

      const currentUrl = urls[i];
      const commandName = resolveDownloadCommand(currentUrl);
      setCurrentQueueItem(i + 1);
      activeQueueItemRef.current = i + 1;

      if (!commandName) {
        failedUrls.push(currentUrl);
        setDownloadItems((prev) => {
          const next = [...prev];
          next[i] = { ...next[i], status: "error" };
          return next;
        });
        continue;
      }

      setDownloadStatus("starting");
      setIsPaused(false);
      setStatusMessage(`Processing ${i + 1}/${urls.length}: ${currentUrl}`);
      setDownloadItems((prev) => {
        const next = [...prev];
        next[i] = { ...next[i], status: "starting" };
        return next;
      });

      try {
        await invoke(commandName, { url: currentUrl });
      } catch (error) {
        if (cancelRequestedRef.current) {
          break;
        }

        failedUrls.push(currentUrl);
        setDownloadStatus("error");
        setStatusMessage(`Failed ${i + 1}/${urls.length}: ${String(error)}`);
        setDownloadItems((prev) => {
          const next = [...prev];
          next[i] = { ...next[i], status: "error" };
          return next;
        });
      }
    }

    setIsBatchRunning(false);
    setIsPaused(false);
    if (cancelRequestedRef.current) {
      setDownloadStatus("cancelled");
      setStatusMessage("Download canceled.");
    } else if (failedUrls.length > 0) {
      setDownloadStatus("error");
      setStatusMessage(`Completed with ${failedUrls.length} error(s).`);
    } else {
      setDownloadStatus("done");
      setStatusMessage(`All ${urls.length} downloads completed.`);
    }
  };

  const toggleDownloadPause = async () => {
    try {
      const paused = await invoke<boolean>("toggle_pause_download");
      setIsPaused(paused);
      setDownloadStatus(paused ? "paused" : "downloading");
      setStatusMessage(paused ? "Download paused." : "Download resumed.");
    } catch (error) {
      setStatusMessage(String(error));
    }
  };

  const cancelActiveDownload = async () => {
    cancelRequestedRef.current = true;

    try {
      await invoke("cancel_download");
      setDownloadStatus("cancelled");
      setIsPaused(false);
      setStatusMessage("Canceling active download...");
    } catch (error) {
      cancelRequestedRef.current = false;
      setStatusMessage(String(error));
    }
  };

  const openDownloadRoot = async () => {
    try {
      await openPath(await resolveDownloadFolderPath());
    } catch (error) {
      setStatusMessage(String(error));
    }
  };

  const openCookiesFile = async () => {
    try {
      const cookiesPath = await invoke<string>("get_cookies_file_path");
      await openPath(cookiesPath);
    } catch (error) {
      setStatusMessage(String(error));
    }
  };

  const openItemDownloadFolder = async (url: string) => {
    try {
      await openPath(await resolveDownloadFolderPath(url));
    } catch (error) {
      setStatusMessage(String(error));
    }
  };

  const openSourceUrl = async (url: string) => {
    try {
      await openUrl(url);
    } catch (error) {
      setStatusMessage(String(error));
    }
  };

  const footerStatusText = isDownloadActive
    ? `[${currentQueueItem}/${queueTotal}] ${statusMessage}`
    : statusMessage;

  return (
    <>
      <main className="flex min-h-screen flex-col gap-6 p-4">
        <DownloadControls
          inputValue={urlInputValue}
          isDownloadActive={isDownloadActive || isMissingCookiesModalOpen}
          isPaused={isPaused}
          onCancel={cancelActiveDownload}
          onInputChange={setUrlInputValue}
          onOpenCookiesFile={openCookiesFile}
          onOpenDownloadRoot={openDownloadRoot}
          onStart={startDownloadQueue}
          onTogglePause={toggleDownloadPause}
        />

        <div className="flex-1">
          <DownloadQueueTable
            currentPage={currentPage}
            downloadItems={downloadItems}
            onOpenFolder={openItemDownloadFolder}
            onOpenLink={openSourceUrl}
            onPageChange={setCurrentPage}
          />
        </div>

        <StatusFooter
          onBuyCoffee={() => openSourceUrl(buyMeACoffeeUrl)}
          statusText={footerStatusText}
        />
      </main>

      <AlertDialog
        isOpen={isMissingCookiesModalOpen}
        onOpenChange={setIsMissingCookiesModalOpen}
      >
        <AlertDialog.Backdrop variant="blur">
          <AlertDialog.Container>
            <AlertDialog.Dialog className="sm:max-w-100">
              <AlertDialog.Header>
                <AlertDialog.Icon status="warning">
                  <LockOpen className="size-5" />
                </AlertDialog.Icon>
                <AlertDialog.Heading>
                  Missing or empty cookies.txt
                </AlertDialog.Heading>
              </AlertDialog.Header>
              <AlertDialog.Body>
                <p>
                  The file cookies.txt was not found, or it is empty. Open the
                  GitHub guide to follow the setup instructions and place valid
                  cookie content in the app folder.
                </p>
                <p>
                  After you finish, press OK to close the app, then open it
                  again.
                </p>
              </AlertDialog.Body>
              <AlertDialog.Footer className="flex-col">
                <Button
                  className="w-full"
                  onPress={() => openSourceUrl(cookiesGuideUrl)}
                  variant="secondary"
                >
                  Open Guide
                </Button>
                <Button
                  className="w-full"
                  onPress={openCookiesFile}
                  variant="secondary"
                >
                  Open cookies.txt
                </Button>
                <Button className="w-full" onPress={quitApp} variant="primary">
                  OK
                </Button>
              </AlertDialog.Footer>
            </AlertDialog.Dialog>
          </AlertDialog.Container>
        </AlertDialog.Backdrop>
      </AlertDialog>
    </>
  );
}

export default App;
