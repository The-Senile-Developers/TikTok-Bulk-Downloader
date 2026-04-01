export interface DownloadItem {
  description: string;
  id: number;
  link: string;
  progress: number;
  status: string;
}

export interface DownloadProgress {
  current_item: number;
  current_title: string;
  detail: string;
  eta: string;
  percent: number;
  speed: string;
  status: string;
  total_items: number;
  username: string;
}
