import { FolderOpen } from "@gravity-ui/icons";
import { Button, Label, TextArea } from "@heroui/react";

interface DownloadControlsProps {
  inputValue: string;
  isDownloadActive: boolean;
  isPaused: boolean;
  onCancel: () => void;
  onInputChange: (value: string) => void;
  onOpenCookiesFile: () => void;
  onOpenDownloadRoot: () => void;
  onStart: () => void;
  onTogglePause: () => void;
}

export function DownloadControls({
  inputValue,
  isDownloadActive,
  isPaused,
  onCancel,
  onInputChange,
  onOpenCookiesFile,
  onOpenDownloadRoot,
  onStart,
  onTogglePause,
}: DownloadControlsProps) {
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-2">
        <Label>URLs</Label>
        <TextArea
          className="max-h-40"
          disabled={isDownloadActive}
          onChange={(e) => onInputChange(e.target.value)}
          placeholder="Enter one or more TikTok URLs (one per line)."
          value={inputValue}
        />
      </div>
      <div className="flex justify-between">
        <div className="flex gap-2">
          <Button
            isDisabled={isDownloadActive}
            onPress={onStart}
            variant="primary"
          >
            {isDownloadActive ? "Downloading..." : "Download"}
          </Button>
          {isDownloadActive && (
            <>
              <Button onPress={onTogglePause} variant="tertiary">
                {isPaused ? "Resume" : "Pause"}
              </Button>
              <Button onPress={onCancel} variant="danger">
                Cancel
              </Button>
            </>
          )}
        </div>
        <div className="flex gap-2">
          <Button onPress={onOpenCookiesFile} variant="secondary">
            Open cookies.txt
          </Button>
          <Button onPress={onOpenDownloadRoot} variant="secondary">
            <FolderOpen />
            Open Download Folder
          </Button>
        </div>
      </div>
    </div>
  );
}
