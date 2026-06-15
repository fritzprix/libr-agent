#!/usr/bin/env python3
"""
LibrAgent Google Calendar client.

Actions:
  list_events   — list events in a time range
  get_event     — fetch one event by ID
  create_event  — create an event
  update_event  — patch an event
  delete_event  — delete an event
  week_summary  — week overview for summaries

All output is JSON on stdout. Errors are JSON on stderr with exit code 1.
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timedelta, timezone
from typing import Any
from zoneinfo import ZoneInfo

from calendar_config import build_calendar_service, json_error


def parse_rfc3339(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def to_api_time(value: str, *, all_day: bool) -> dict[str, str]:
    if all_day:
        return {"date": value.split("T", 1)[0] if "T" in value else value}
    dt = parse_rfc3339(value)
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return {"dateTime": dt.isoformat()}


def event_summary(item: dict[str, Any]) -> dict[str, Any]:
    start = item.get("start", {})
    end = item.get("end", {})
    return {
        "id": item.get("id"),
        "summary": item.get("summary", "(no title)"),
        "start": start.get("dateTime") or start.get("date"),
        "end": end.get("dateTime") or end.get("date"),
        "location": item.get("location"),
        "status": item.get("status"),
        "htmlLink": item.get("htmlLink"),
    }


def action_list_events(args: argparse.Namespace) -> dict[str, Any]:
    service = build_calendar_service()
    params: dict[str, Any] = {
        "calendarId": args.calendar_id,
        "singleEvents": True,
        "orderBy": "startTime",
        "maxResults": args.limit,
    }
    if args.from_time:
        params["timeMin"] = to_api_time(args.from_time, all_day=False).get(
            "dateTime", args.from_time
        )
    if args.to_time:
        params["timeMax"] = to_api_time(args.to_time, all_day=False).get(
            "dateTime", args.to_time
        )

    result = service.events().list(**params).execute()
    items = [event_summary(item) for item in result.get("items", [])]
    return {"action": "list_events", "count": len(items), "events": items}


def action_get_event(args: argparse.Namespace) -> dict[str, Any]:
    service = build_calendar_service()
    item = (
        service.events()
        .get(calendarId=args.calendar_id, eventId=args.event_id)
        .execute()
    )
    return {"action": "get_event", "event": item}


def action_create_event(args: argparse.Namespace) -> dict[str, Any]:
    service = build_calendar_service()
    body: dict[str, Any] = {
        "summary": args.title,
        "start": to_api_time(args.start, all_day=args.all_day),
        "end": to_api_time(args.end, all_day=args.all_day),
    }
    if args.description:
        body["description"] = args.description
    if args.location:
        body["location"] = args.location
    if args.attendees:
        body["attendees"] = [
            {"email": email.strip()}
            for email in args.attendees.split(",")
            if email.strip()
        ]

    created = (
        service.events()
        .insert(calendarId=args.calendar_id, body=body, sendUpdates=args.send_updates)
        .execute()
    )
    return {"action": "create_event", "event": event_summary(created)}


def action_update_event(args: argparse.Namespace) -> dict[str, Any]:
    service = build_calendar_service()
    existing = (
        service.events()
        .get(calendarId=args.calendar_id, eventId=args.event_id)
        .execute()
    )
    if args.title:
        existing["summary"] = args.title
    if args.description is not None:
        existing["description"] = args.description
    if args.location is not None:
        existing["location"] = args.location
    if args.start:
        existing["start"] = to_api_time(args.start, all_day=args.all_day)
    if args.end:
        existing["end"] = to_api_time(args.end, all_day=args.all_day)

    updated = (
        service.events()
        .update(
            calendarId=args.calendar_id,
            eventId=args.event_id,
            body=existing,
            sendUpdates=args.send_updates,
        )
        .execute()
    )
    return {"action": "update_event", "event": event_summary(updated)}


def action_delete_event(args: argparse.Namespace) -> dict[str, Any]:
    service = build_calendar_service()
    service.events().delete(
        calendarId=args.calendar_id,
        eventId=args.event_id,
        sendUpdates=args.send_updates,
    ).execute()
    return {"action": "delete_event", "event_id": args.event_id, "deleted": True}


def action_week_summary(args: argparse.Namespace) -> dict[str, Any]:
    tz = ZoneInfo(args.timezone)
    if args.start_date:
        anchor = datetime.fromisoformat(args.start_date).replace(tzinfo=tz)
    else:
        now = datetime.now(tz)
        anchor = now - timedelta(days=now.weekday())

    start = anchor.replace(hour=0, minute=0, second=0, microsecond=0)
    end = start + timedelta(days=7)

    service = build_calendar_service()
    result = (
        service.events()
        .list(
            calendarId=args.calendar_id,
            timeMin=start.isoformat(),
            timeMax=end.isoformat(),
            singleEvents=True,
            orderBy="startTime",
            maxResults=100,
        )
        .execute()
    )
    items = [event_summary(item) for item in result.get("items", [])]
    return {
        "action": "week_summary",
        "timezone": args.timezone,
        "range_start": start.isoformat(),
        "range_end": end.isoformat(),
        "count": len(items),
        "events": items,
    }


ACTIONS = {
    "list_events": action_list_events,
    "get_event": action_get_event,
    "create_event": action_create_event,
    "update_event": action_update_event,
    "delete_event": action_delete_event,
    "week_summary": action_week_summary,
}


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="LibrAgent Google Calendar CLI")
    parser.add_argument("--action", required=True, choices=sorted(ACTIONS))
    parser.add_argument("--calendar-id", default="primary")
    parser.add_argument("--send-updates", default="none", choices=["all", "externalOnly", "none"])
    parser.add_argument("--event-id")
    parser.add_argument("--title")
    parser.add_argument("--start")
    parser.add_argument("--end")
    parser.add_argument("--description")
    parser.add_argument("--location")
    parser.add_argument("--attendees")
    parser.add_argument("--all-day", action="store_true")
    parser.add_argument("--from", dest="from_time")
    parser.add_argument("--to", dest="to_time")
    parser.add_argument("--limit", type=int, default=25)
    parser.add_argument("--timezone", default="UTC")
    parser.add_argument("--start-date")
    return parser


def validate_args(args: argparse.Namespace) -> None:
    if args.action == "get_event" and not args.event_id:
        raise ValueError("--event-id is required for get_event")
    if args.action == "create_event":
        if not args.title or not args.start or not args.end:
            raise ValueError("--title, --start, and --end are required for create_event")
    if args.action in {"update_event", "delete_event"} and not args.event_id:
        raise ValueError(f"--event-id is required for {args.action}")


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()

    try:
        validate_args(args)
        payload = ACTIONS[args.action](args)
    except Exception as exc:
        json_error(str(exc))
        return 1

    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
