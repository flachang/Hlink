import { QRCodeSVG } from "qrcode.react";
import { X, Copy, Check } from "lucide-react";
import { useState } from "react";

interface Props {
  ip: string;
  port: number;
  onClose: () => void;
}

export default function QrConnect({ ip, port, onClose }: Props) {
  const [copied, setCopied] = useState(false);

  // 连接 URL：手机 Hlink App 打开后扫码即可自动连接
  const wsUrl = `hlink://connect?addr=${ip}:${port}`;
  // 同时展示 ws:// 地址便于手动输入
  const wsAddr = `${ip}:${port}`;

  const handleCopy = async () => {
    await navigator.clipboard.writeText(wsAddr);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="qr-overlay" onClick={onClose}>
      <div className="qr-card" onClick={(e) => e.stopPropagation()}>
        {/* 标题栏 */}
        <div className="qr-header">
          <span>扫码连接此设备</span>
          <button className="btn-icon" onClick={onClose}>
            <X size={16} />
          </button>
        </div>

        {/* 二维码 */}
        <div className="qr-body">
          <div className="qr-code-wrap">
            <QRCodeSVG
              value={wsUrl}
              size={200}
              bgColor="#1a1d27"
              fgColor="#e2e6f3"
              level="M"
              includeMargin={true}
            />
          </div>

          <p className="qr-tip">在手机 Hlink App 中扫描上方二维码</p>

          {/* IP 地址 + 复制 */}
          <div className="qr-addr-row">
            <code className="qr-addr">{wsAddr}</code>
            <button className="btn-copy" onClick={handleCopy}>
              {copied ? <Check size={14} /> : <Copy size={14} />}
              {copied ? "已复制" : "复制"}
            </button>
          </div>

          <p className="qr-hint">
            确保手机与此设备连接到<strong>同一 Wi-Fi</strong>
          </p>
        </div>
      </div>
    </div>
  );
}
