import {
  Button,
  EmptyState,
  Label,
  Pagination,
  ProgressBar,
  Table,
} from "@heroui/react";
import { Icon } from "@iconify/react";

import type { DownloadItem } from "../types";

const TABLE_COLUMNS = [
  { id: "description", name: "Description" },
  { id: "progress", name: "Progress" },
  { id: "actions", name: "Actions" },
] as const;

const ROWS_PER_PAGE = 4;

interface DownloadQueueTableProps {
  currentPage: number;
  downloadItems: DownloadItem[];
  onOpenFolder: (url: string) => void;
  onOpenLink: (url: string) => void;
  onPageChange: (page: number) => void;
}

export function DownloadQueueTable({
  currentPage,
  downloadItems,
  onOpenFolder,
  onOpenLink,
  onPageChange,
}: DownloadQueueTableProps) {
  const totalPages = Math.max(
    1,
    Math.ceil(downloadItems.length / ROWS_PER_PAGE),
  );
  const pageNumbers = Array.from({ length: totalPages }, (_, i) => i + 1);
  const start = (currentPage - 1) * ROWS_PER_PAGE;
  const visibleDownloadItems = downloadItems.slice(
    start,
    start + ROWS_PER_PAGE,
  );
  const visibleRangeEnd = Math.min(
    currentPage * ROWS_PER_PAGE,
    downloadItems.length,
  );
  const visibleRangeStart =
    downloadItems.length === 0 ? 0 : (currentPage - 1) * ROWS_PER_PAGE + 1;

  const getLabelForStatus = (status: string) => {
    switch (status) {
      case "cancelled":
        return "Canceled";
      case "done":
        return "Completed";
      case "downloading":
        return "Downloading";
      case "error":
        return "Error";
      case "paused":
        return "Paused";
      case "pending":
        return "Waiting";
      case "starting":
        return "Starting";
      default:
        return status || "Unknown";
    }
  };

  return (
    <Table>
      <Table.ScrollContainer>
        <Table.Content aria-label="Download table" className="min-w-150">
          <Table.Header columns={TABLE_COLUMNS}>
            {(column) => (
              <Table.Column isRowHeader={column.id === "description"}>
                {column.name}
              </Table.Column>
            )}
          </Table.Header>
          <Table.Body
            items={visibleDownloadItems}
            renderEmptyState={() => (
              <EmptyState className="flex h-full w-full flex-col items-center justify-center gap-4 text-center">
                <Icon className="size-6 text-muted" icon="gravity-ui:tray" />
                <span className="text-sm text-muted">
                  Your download queue will appear here.
                </span>
              </EmptyState>
            )}
          >
            {(item) => (
              <Table.Row>
                <Table.Collection items={TABLE_COLUMNS}>
                  {(column) => (
                    <Table.Cell>
                      {column.id === "description" && (
                        <div
                          className="max-w-75 overflow-hidden text-ellipsis whitespace-nowrap"
                          title={item.description}
                        >
                          {item.description}
                        </div>
                      )}
                      {column.id === "progress" && (
                        <ProgressBar
                          aria-label="Download progress"
                          className="w-64"
                          value={item.progress}
                        >
                          <Label>{getLabelForStatus(item.status)}</Label>
                          <ProgressBar.Output />
                          <ProgressBar.Track>
                            <ProgressBar.Fill />
                          </ProgressBar.Track>
                        </ProgressBar>
                      )}
                      {column.id === "actions" && (
                        <div className="flex gap-2">
                          <Button
                            onPress={() => onOpenFolder(item.link)}
                            size="sm"
                            variant="secondary"
                          >
                            Open Folder
                          </Button>
                          <Button
                            onPress={() => onOpenLink(item.link)}
                            size="sm"
                            variant="tertiary"
                          >
                            Open URL
                          </Button>
                        </div>
                      )}
                    </Table.Cell>
                  )}
                </Table.Collection>
              </Table.Row>
            )}
          </Table.Body>
        </Table.Content>
      </Table.ScrollContainer>
      {downloadItems.length > 0 && (
        <Table.Footer>
          <Pagination size="sm">
            <Pagination.Summary>
              {visibleRangeStart} to {visibleRangeEnd} of {downloadItems.length}{" "}
              results
            </Pagination.Summary>
            <Pagination.Content>
              <Pagination.Item>
                <Pagination.Previous
                  isDisabled={currentPage === 1}
                  onPress={() => onPageChange(Math.max(1, currentPage - 1))}
                >
                  <Pagination.PreviousIcon />
                  Previous
                </Pagination.Previous>
              </Pagination.Item>
              {pageNumbers.map((pageNumber) => (
                <Pagination.Item key={pageNumber}>
                  <Pagination.Link
                    isActive={pageNumber === currentPage}
                    onPress={() => onPageChange(pageNumber)}
                  >
                    {pageNumber}
                  </Pagination.Link>
                </Pagination.Item>
              ))}
              <Pagination.Item>
                <Pagination.Next
                  isDisabled={currentPage === totalPages}
                  onPress={() =>
                    onPageChange(Math.min(totalPages, currentPage + 1))
                  }
                >
                  Next
                  <Pagination.NextIcon />
                </Pagination.Next>
              </Pagination.Item>
            </Pagination.Content>
          </Pagination>
        </Table.Footer>
      )}
    </Table>
  );
}
