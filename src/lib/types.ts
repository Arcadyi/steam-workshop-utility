export interface GameEntry {
    appid: string;
    name: string;
    path: string;
    build_id: string;

    icon?: string | null;
    header?: string | null;
    num_items?: number;
    num_ood?: number;
}

export type ItemStatus = "Unknown" | "UpToDate" | "OutOfDate";

export interface WorkshopItem {
    item_id: string;
    name: string | null;
    path: string;
    local_timestamp: number | null;
    remote_timestamp: number | null;
    disk_size: number;
    status: ItemStatus;
    incompatible: boolean;
    supported_versions: string[];
    preview_url: string | null;
    selected: boolean;
}