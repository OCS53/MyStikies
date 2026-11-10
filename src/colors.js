export const COLORS = [
  { id: "sun", name: "햇살" },
  { id: "peach", name: "복숭아" },
  { id: "rose", name: "장미" },
  { id: "leaf", name: "잎" },
  { id: "sky", name: "하늘" },
  { id: "lilac", name: "라일락" },
  { id: "sand", name: "모래" },
  { id: "ink", name: "먹" },
];

export function formatWhen(ms) {
  if (!ms) return "";
  const date = new Date(ms);
  const now = new Date();
  const time = date.toLocaleTimeString("ko-KR", { hour: "numeric", minute: "2-digit" });
  if (date.toDateString() === now.toDateString()) return `오늘 ${time}`;
  const tomorrow = new Date(now);
  tomorrow.setDate(now.getDate() + 1);
  if (date.toDateString() === tomorrow.toDateString()) return `내일 ${time}`;
  return `${date.getMonth() + 1}월 ${date.getDate()}일 ${time}`;
}

export function formatBytes(bytes) {
  const size = Number(bytes) || 0;
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}
