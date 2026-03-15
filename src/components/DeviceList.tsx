import { Monitor, Smartphone, Wifi, WifiOff } from "lucide-react";
import { PeerDevice } from "../types";

interface Props {
  devices: PeerDevice[];
  onConnect: (addr: string) => void;
}

export default function DeviceList({ devices, onConnect }: Props) {
  return (
    <div className="section">
      <div className="section-header">
        <Wifi size={16} />
        <span>局域网设备</span>
        <span className="badge">{devices.length}</span>
      </div>

      {devices.length === 0 ? (
        <div className="empty-state">
          <WifiOff size={32} className="empty-icon" />
          <p>正在扫描局域网中的 Hlink 设备…</p>
          <p className="hint">确保其他设备与此设备连接到同一 Wi-Fi</p>
        </div>
      ) : (
        <ul className="device-list">
          {devices.map((device) => (
            <li key={device.id} className="device-item">
              <div className="device-icon">
                {device.name.toLowerCase().includes("phone") ||
                device.name.toLowerCase().includes("iphone") ||
                device.name.toLowerCase().includes("android") ? (
                  <Smartphone size={20} />
                ) : (
                  <Monitor size={20} />
                )}
              </div>
              <div className="device-info">
                <span className="device-name">{device.name}</span>
                <span className="device-addr">
                  {device.addresses[0]}:{device.port}
                </span>
              </div>
              <button
                className="btn-connect"
                onClick={() =>
                  onConnect(`${device.addresses[0]}:${device.port}`)
                }
              >
                连接
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
