import { STATUS, type Status } from './model';

export const STATUS_ICONS: Record<Status,string> = {todo:'⭕',doing:'⌛',done:'✅',missed:'❌'};
export function statusOption(value:string,label:string):Status|null {
  const status=value==='active'&&label===STATUS.doing?'doing':value as Status;
  return Object.hasOwn(STATUS,status)&&(label===STATUS[status]||label===`${STATUS_ICONS[status]} ${STATUS[status]}`)?status:null;
}
export default function StatusIcon({value}:{value:Status}) {
  return <span className="task-status-icon" role="img" aria-label={STATUS[value]} title={STATUS[value]}>{STATUS_ICONS[value]}</span>;
}
