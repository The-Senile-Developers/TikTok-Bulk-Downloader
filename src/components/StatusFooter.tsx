import { ThumbsUp } from "@gravity-ui/icons";
import { Button } from "@heroui/react";

interface StatusFooterProps {
  onBuyCoffee: () => void;
  statusText: string;
}

export function StatusFooter({ onBuyCoffee, statusText }: StatusFooterProps) {
  return (
    <div className="flex items-end justify-between">
      <p className="flex-1">{statusText}</p>
      <Button onPress={onBuyCoffee} variant="outline">
        <ThumbsUp />
        Buy Me a Coffee
      </Button>
    </div>
  );
}
