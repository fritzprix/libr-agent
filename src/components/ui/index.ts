// Shadcn components
export { Badge } from './badge';
export { Button } from './button';
export { Input } from './input';
export { Textarea } from './textarea';
export {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from './card';
export {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from './dialog';
export {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  DropdownMenuSeparator,
} from './dropdown-menu';
export { Separator } from './separator';
export {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from './tooltip';
export { Tabs, TabsContent, TabsList, TabsTrigger } from './tabs';

// Custom components that don't have Shadcn equivalents
export { Dropdown } from './Dropdown';
export { default as BadgeLegacy } from './BadgeLegacy';
export { default as ButtonLegacy } from './ButtonLegacy';
export { default as FileAttachment } from './FileAttachment';
export { default as InputWithLabel } from './InputWithLabel';
export { default as LoadingSpinner } from './LoadingSpinner';
export { default as Modal } from './Modal';
export { default as StatusIndicator } from './StatusIndicator';
export { default as TextareaWithLabel } from './TextareaWithLabel';

// Model picker components (from parent directory)
export {
  CompactModelPicker,
  TerminalModelPicker,
} from '../../features/chat/ModelPicker';
