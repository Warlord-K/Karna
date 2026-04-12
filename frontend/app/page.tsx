import { authDisabled } from "@/auth";
import HomePage from "./home";

export default function Page() {
  return <HomePage authDisabled={authDisabled} />;
}
