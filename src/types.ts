export interface PeerDevice {
  id: string;
  name: string;
  addresses: string[];
  port: number;
}

export type ClipKind = "text" | "image" | "file";

export interface HistoryEntry {
  kind: ClipKind;
  from: string;
  preview: string;
  timestamp: number;
  file_path?: string;  // 图片文件路径
  has_image?: boolean; // 是否有图片数据可预览（true = 可点击放大）
}

export interface ClipPayload {
  type: ClipKind;
  from: string;
  payload: string;
  width?: number;
  height?: number;
  filename?: string;
}
